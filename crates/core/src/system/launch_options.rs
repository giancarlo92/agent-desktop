use std::collections::BTreeMap;
use std::path::PathBuf;

/// Options accepted by `launch_app`. `attach_if_running` defaults to `true`,
/// which preserves the behavior of attaching to an already-running instance
/// instead of failing. Set it to `false` to
/// require a fresh launch: the adapter then fails with a structured error
/// naming the running pid instead of attaching.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub timeout_ms: u64,
    pub attach_if_running: bool,
    /// Brings the application forward so it presents a window. A document-based
    /// application creates its first window in response to activation, so a
    /// caller that needs a window has to ask for one; waiting without asking
    /// waits for an event that never fires.
    pub activate: bool,
    /// The resolved, concrete DevTools protocol port. Core resolves a
    /// requested `0` to a real free port before the adapter ever sees it, so
    /// by the time `launch_app` runs, this is either absent or one exact port.
    pub cdp_port: Option<u16>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            timeout_ms: 5_000,
            attach_if_running: true,
            activate: false,
            cdp_port: None,
        }
    }
}

/// Takes the pages to open out of a browser's arguments: every argument that
/// is not a switch, and the `--new-window` that would go with them. With
/// none, the browser opens a blank page.
pub fn take_pages(args: &mut Vec<String>) -> Vec<String> {
    let mut pages = Vec::new();
    args.retain(|arg| {
        if arg == "--new-window" {
            return false;
        }
        if arg.starts_with('-') {
            return true;
        }
        pages.push(arg.clone());
        false
    });
    if pages.is_empty() {
        pages.push("about:blank".to_owned());
    }
    pages
}

/// Whether the launch arguments give the program a profile directory of its
/// own (`--user-data-dir`, as Chromium browsers take it): such an instance is
/// separate from any other running instance of the same program.
pub fn has_own_profile(args: &[String]) -> bool {
    args.iter().any(|arg| arg.starts_with("--user-data-dir"))
}

#[cfg(test)]
#[path = "launch_options_tests.rs"]
mod tests;
