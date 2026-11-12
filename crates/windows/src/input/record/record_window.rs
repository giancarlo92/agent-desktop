//! Maps the native window under an action to the window `list-windows`
//! reports, so a recorded `window.id` is one the other commands accept.
//!
//! A dropdown list, a menu or a tooltip is its own top-level popup that
//! `list-windows` does not list. Such a popup is attributed first to the
//! window that owns it and otherwise to a listed window of the same process,
//! so an `--app` or `--window-id` filter keeps the actions taken inside it.

use agent_desktop_core::{Deadline, WindowFilter, WindowInfo};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, GetWindowThreadProcessId};

use crate::system::window::window_ops::list_windows_live;

const GA_ROOT: u32 = 2;
const GA_ROOTOWNER: u32 = 3;
const WINDOW_LIST_TIMEOUT_MS: u64 = 2_000;
const LIST_ATTEMPTS: u32 = 3;
const LIST_RETRY_PAUSE: std::time::Duration = std::time::Duration::from_millis(60);

/// The listed window an action belongs to, and the id of the window that
/// owns it when that differs (a dialog or popup of the listed window).
pub(super) struct ResolvedWindow {
    pub(super) window: Option<WindowInfo>,
    pub(super) owner_id: Option<String>,
}

#[derive(Default)]
pub(super) struct WindowBook {
    windows: Vec<WindowInfo>,
}

impl WindowBook {
    pub(super) fn resolve(&mut self, handle: isize, preferred_id: Option<&str>) -> ResolvedWindow {
        let root = ancestor(handle, GA_ROOT);
        let owner = ancestor(handle, GA_ROOTOWNER);
        let owner_id = (owner != 0 && owner != root).then(|| window_id_of(owner));
        let window = self
            .listed(root)
            .or_else(|| self.listed(owner))
            .or_else(|| self.same_process(root, preferred_id));
        ResolvedWindow { window, owner_id }
    }

    fn listed(&mut self, handle: isize) -> Option<WindowInfo> {
        if handle == 0 {
            return None;
        }
        let id = window_id_of(handle);
        if let Some(window) = self.find(&id) {
            return Some(window);
        }
        self.refresh();
        self.find(&id)
    }

    fn same_process(&self, handle: isize, preferred_id: Option<&str>) -> Option<WindowInfo> {
        if handle == 0 {
            return None;
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(handle as _, &mut pid) };
        let siblings: Vec<&WindowInfo> = self.windows.iter().filter(|w| w.pid == pid).collect();
        siblings
            .iter()
            .find(|window| preferred_id == Some(window.id.as_str()))
            .or_else(|| siblings.iter().find(|window| window.state.is_focused))
            .or_else(|| siblings.first())
            .map(|window| (*window).clone())
    }

    fn find(&self, id: &str) -> Option<WindowInfo> {
        self.windows.iter().find(|window| window.id == id).cloned()
    }

    /// Re-reads the window inventory. A listing can fail while windows are
    /// being created or destroyed, so a failed read is retried briefly before
    /// the last good inventory is kept.
    fn refresh(&mut self) {
        for attempt in 0..LIST_ATTEMPTS {
            let listed = Deadline::detached_after(WINDOW_LIST_TIMEOUT_MS)
                .and_then(|deadline| list_windows_live(&WindowFilter::default(), deadline));
            match listed {
                Ok(windows) => {
                    self.windows = windows;
                    return;
                }
                Err(error) => {
                    tracing::debug!(attempt, code = ?error.code, "record: window listing failed");
                    std::thread::sleep(LIST_RETRY_PAUSE);
                }
            }
        }
    }
}

/// The `w-<hwnd>` id `list-windows` reports for a top-level window.
pub(super) fn window_id_of(handle: isize) -> String {
    format!("w-{}", handle as usize)
}

/// The window's root (`GA_ROOT`) or owned root (`GA_ROOTOWNER`), or the
/// handle itself when the walk has no answer.
pub(super) fn ancestor(handle: isize, kind: u32) -> isize {
    if handle == 0 {
        return 0;
    }
    let found = unsafe { GetAncestor(handle as _, kind) } as isize;
    if found == 0 { handle } else { found }
}

pub(super) fn root_of_handle(handle: isize) -> isize {
    ancestor(handle, GA_ROOT)
}
