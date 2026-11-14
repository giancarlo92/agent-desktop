//! Writing a value without bringing the target's window to the front.
//!
//! `ValuePattern.SetValue` sends no input, but the classic Win32 edit control
//! (also behind WinForms text boxes) activates its window when it receives
//! one: whatever the person is typing at that moment would land in it. A
//! headless write into such a control goes as `WM_SETTEXT` to the control's
//! own window instead, which never activates anything.
//!
//! Every other provider keeps `ValuePattern.SetValue`, under `focus_guard`.
//! Chromium (Chrome, Edge, Electron) is one: it too activates its window as
//! the value arrives, and the guard gives the foreground back within
//! milliseconds. Typing into Chromium by window messages is not used: while
//! the window believes it is active, keys the person is pressing reach it.

use agent_desktop_core::{ActionStep, AdapterError, Deadline, ErrorCode, StepMechanism};
use windows::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW, WM_SETTEXT,
};

use crate::actions::value_write::gated_pattern_value_equals;
use crate::tree::element::UIAElement;

const SET_TEXT_LABEL: &str = "WindowMessage.WM_SETTEXT";
const SET_TEXT_TIMEOUT_MS: u32 = 2_000;

/// Writes `value` into `element` without activating its window, or `None`
/// when the element is not a Win32 edit control.
pub(crate) fn background_set_value(
    element: &UIAElement,
    value: &str,
    deadline: Deadline,
) -> Result<Option<Vec<ActionStep>>, AdapterError> {
    match win32_edit(element) {
        Some(edit) => set_text(element, edit, value, deadline).map(|step| Some(vec![step])),
        None => Ok(None),
    }
}
/// The element's own window handle when it is a Win32 edit control
/// (`Edit`, `RichEdit*`, `WindowsForms10.EDIT.*`).
fn win32_edit(element: &UIAElement) -> Option<isize> {
    let handle = element.0.get_native_window_handle().ok()?;
    let hwnd: HWND = handle.into();
    let hwnd = hwnd.0 as isize;
    if hwnd == 0 {
        return None;
    }
    let mut class = [0u16; 128];
    let length = unsafe { GetClassNameW(hwnd as _, class.as_mut_ptr(), class.len() as i32) };
    if length <= 0 {
        return None;
    }
    let class = String::from_utf16_lossy(&class[..length as usize]).to_ascii_lowercase();
    let edit = class == "edit"
        || class.starts_with("richedit")
        || class.starts_with("windowsforms10.edit");
    edit.then_some(hwnd)
}

fn set_text(
    element: &UIAElement,
    hwnd: isize,
    value: &str,
    deadline: Deadline,
) -> Result<ActionStep, AdapterError> {
    deadline.ensure_remaining()?;
    let mut text: Vec<u16> = value.encode_utf16().collect();
    text.push(0);
    let mut result = 0usize;
    let sent = unsafe {
        SendMessageTimeoutW(
            hwnd as _,
            WM_SETTEXT,
            0,
            text.as_ptr() as isize,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            SET_TEXT_TIMEOUT_MS,
            &mut result,
        )
    };
    text.iter_mut().for_each(|unit| *unit = 0);
    if sent == 0 || result == 0 {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "The text box did not accept the text (WM_SETTEXT)",
        ));
    }
    let verified = gated_pattern_value_equals(element, value)?;
    Ok(ActionStep::succeeded(SET_TEXT_LABEL)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(verified.unwrap_or(false)))
}
