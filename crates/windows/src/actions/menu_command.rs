//! A menu-bar command, run without opening the menu.
//!
//! A classic menu bar (the window's `HMENU`) only opens in the window in
//! front, so a background action cannot open it. Choosing an item ends in one
//! message to the window, `WM_COMMAND` with the item's id; that message is
//! sent here directly. Apps that fill a menu when it opens (Eclipse and
//! DBeaver do) are told it opens first (`WM_INITMENUPOPUP`), as they would be
//! when a person opens it, and that it closed after the command.

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, WindowInfo};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMenu, GetMenuItemCount, GetMenuItemInfoW, HMENU, MENUITEMINFOW, MFS_DISABLED, MFT_SEPARATOR,
    MIIM_FTYPE, MIIM_ID, MIIM_STATE, MIIM_STRING, MIIM_SUBMENU, PostMessageW, SMTO_ABORTIFHUNG,
    SMTO_BLOCK, SendMessageTimeoutW, WM_COMMAND, WM_INITMENU, WM_INITMENUPOPUP, WM_UNINITMENUPOPUP,
};

const SEND_TIMEOUT_MS: u32 = 2_000;

struct Item {
    text: String,
    id: u32,
    submenu: HMENU,
    disabled: bool,
    position: u32,
}

/// Runs the item at `path` of `window`'s menu bar. Returns the item names as
/// the menu shows them.
pub(crate) fn invoke(
    window: &WindowInfo,
    path: &[String],
    deadline: Deadline,
) -> Result<Vec<String>, AdapterError> {
    let hwnd = crate::system::window::window_ops::parse_window_id(&window.id)?;
    let bar = unsafe { GetMenu(hwnd as _) };
    if bar.is_null() {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "This window has no classic menu bar",
        )
        .with_suggestion(
            "Its menus are part of the window: find the menu item with snapshot and click it",
        ));
    }
    send(hwnd, WM_INITMENU, bar as usize, 0);
    let mut menu = bar;
    let mut opened = Vec::new();
    let mut chosen = Vec::new();
    for (depth, wanted) in path.iter().enumerate() {
        deadline.ensure_remaining()?;
        let items = items(menu);
        let Some(item) = items.iter().find(|item| same(&item.text, wanted)) else {
            close(hwnd, &opened);
            let names: Vec<&str> = items.iter().map(|item| item.text.as_str()).collect();
            return Err(AdapterError::new(
                ErrorCode::ElementNotFound,
                format!("The menu has no item '{wanted}'"),
            )
            .with_details(serde_json::json!({ "kind": "no_such_menu_item", "items": names }))
            .with_suggestion("Choose one of details.items"));
        };
        chosen.push(item.text.clone());
        let last = depth + 1 == path.len();
        if item.disabled {
            close(hwnd, &opened);
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!("The menu item '{}' is disabled now", item.text),
            ));
        }
        if !last {
            if item.submenu.is_null() {
                close(hwnd, &opened);
                return Err(AdapterError::new(
                    ErrorCode::ElementNotFound,
                    format!("The menu item '{}' has no submenu", item.text),
                ));
            }
            // Lets an app that fills its menus as they open (SWT, Eclipse) do it now.
            send(
                hwnd,
                WM_INITMENUPOPUP,
                item.submenu as usize,
                item.position as isize,
            );
            opened.push(item.submenu);
            menu = item.submenu;
            continue;
        }
        if !item.submenu.is_null() {
            close(hwnd, &opened);
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                format!("'{}' opens a submenu: name one of its items too", item.text),
            ));
        }
        // Posted, not sent: a command that opens a dialog must not keep this call waiting.
        if unsafe { PostMessageW(hwnd as _, WM_COMMAND, item.id as usize & 0xFFFF, 0) } == 0 {
            close(hwnd, &opened);
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "The window did not accept the menu command (WM_COMMAND)",
            ));
        }
        close(hwnd, &opened);
        return Ok(chosen);
    }
    Ok(chosen)
}

/// Tells the window the submenus "opened" here closed again, after the command
/// already queued (posted, so they arrive in order).
fn close(hwnd: isize, opened: &[HMENU]) {
    for menu in opened.iter().rev() {
        unsafe { PostMessageW(hwnd as _, WM_UNINITMENUPOPUP, *menu as usize, 0) };
    }
}

fn send(hwnd: isize, message: u32, wparam: usize, lparam: isize) {
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            hwnd as _,
            message,
            wparam,
            lparam,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            SEND_TIMEOUT_MS,
            &mut result,
        )
    };
}

fn items(menu: HMENU) -> Vec<Item> {
    let count = unsafe { GetMenuItemCount(menu) }.max(0) as u32;
    (0..count)
        .filter_map(|position| item(menu, position))
        .collect()
}

fn item(menu: HMENU, position: u32) -> Option<Item> {
    let mut info: MENUITEMINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<MENUITEMINFOW>() as u32;
    info.fMask = MIIM_STRING | MIIM_ID | MIIM_SUBMENU | MIIM_STATE | MIIM_FTYPE;
    if unsafe { GetMenuItemInfoW(menu, position, 1, &mut info) } == 0
        || info.fType & MFT_SEPARATOR != 0
    {
        return None;
    }
    let mut text = vec![0u16; info.cch as usize + 1];
    info.dwTypeData = text.as_mut_ptr();
    info.cch += 1;
    if unsafe { GetMenuItemInfoW(menu, position, 1, &mut info) } == 0 {
        return None;
    }
    let length = text
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(text.len());
    Some(Item {
        text: display_name(&String::from_utf16_lossy(&text[..length])),
        id: info.wID,
        submenu: info.hSubMenu,
        disabled: info.fState & MFS_DISABLED != 0,
        position,
    })
}

/// The name as a person reads it: no `&` shortcut marks, no `\t` key text.
pub(crate) fn display_name(raw: &str) -> String {
    let label = raw.split('\t').next().unwrap_or_default();
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if chars.peek() == Some(&'&') {
                out.push('&');
                chars.next();
            }
            continue;
        }
        out.push(c);
    }
    out.trim().to_owned()
}

/// Whether a menu name matches what was asked: ignoring case, shortcut marks
/// and a trailing `...`.
pub(crate) fn same(shown: &str, wanted: &str) -> bool {
    let plain = |text: &str| {
        display_name(text)
            .trim_end_matches('…')
            .trim_end_matches("...")
            .trim()
            .to_lowercase()
    };
    plain(shown) == plain(wanted)
}

#[cfg(test)]
mod tests {
    use super::{display_name, same};

    #[test]
    fn menu_names_read_as_a_person_reads_them() {
        assert_eq!(display_name("&File"), "File");
        assert_eq!(display_name("New SQL script\tCtrl+]"), "New SQL script");
        assert_eq!(display_name("Save && Close"), "Save & Close");
        assert!(same("Save &As...\tCtrl+Shift+S", "save as"));
        assert!(same("Export…", "Export..."));
        assert!(!same("Save", "Save As"));
    }
}
