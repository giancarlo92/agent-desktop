//! Keeps the person's window in front while a headless action runs.
//!
//! A semantic UI Automation call never sends input, but some providers
//! activate their own window when they receive one: Chromium (Chrome, Edge,
//! Electron) does it about 30 ms after a click or a value arrives, and keys the
//! person types at that moment would land in it. A thread of its own watches
//! the foreground from the start of every headless action until `WATCH` after
//! it, and when the target's process has taken it, gives it back to the window
//! the person had in front, within milliseconds.
//!
//! Two other ways were measured and left out: dropping Chromium's injected
//! "foreground" key (it then waits 2 s and comes forward anyway), and
//! `LockSetForegroundWindow` (Chromium still came forward).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, MSG, PM_NOREMOVE,
    PeekMessageW, SetForegroundWindow,
};

/// How long after the action the target's activation is still watched for:
/// Chromium can activate its window a while after the call returns.
const WATCH: Duration = Duration::from_millis(1_500);
const POLL: Duration = Duration::from_millis(1);

fn process_of(window: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    pid
}

/// Runs `action` and keeps the foreground where the person had it, when the
/// action's target belongs to `target_pid` and was not already in front.
pub(crate) fn guarded<T>(target_pid: Option<u32>, action: impl FnOnce() -> T) -> T {
    let before = unsafe { GetForegroundWindow() };
    let Some(target_pid) = target_pid.filter(|&pid| pid != 0) else {
        return action();
    };
    if before.is_null() || process_of(before) == target_pid {
        return action();
    }
    let debug = std::env::var_os("AGENT_DESKTOP_GUARD_DEBUG").is_some();
    let done = Arc::new(AtomicBool::new(false));
    let watcher = {
        let done = done.clone();
        let before = before as isize;
        std::thread::spawn(move || watch(before as HWND, target_pid, &done, debug))
    };
    let acting = Instant::now();
    let result = action();
    if debug {
        eprintln!("guard: action took {:?}", acting.elapsed());
    }
    done.store(true, Ordering::SeqCst);
    let _ = watcher.join();
    result
}

/// Gives the foreground back to `before` whenever the target's process takes
/// it, until `WATCH` after the action is done.
fn watch(before: HWND, target_pid: u32, done: &AtomicBool, debug: bool) {
    let started = Instant::now();
    let mut finished: Option<Instant> = None;
    loop {
        let now = unsafe { GetForegroundWindow() };
        if !now.is_null() && now != before && process_of(now) == target_pid {
            give_back(before, now);
            if debug {
                let back = unsafe { GetForegroundWindow() } == before;
                eprintln!(
                    "guard: taken at {:?}, given back: {back}",
                    started.elapsed()
                );
            }
        }
        match finished {
            Some(at) if at.elapsed() >= WATCH => return,
            None if done.load(Ordering::SeqCst) => finished = Some(Instant::now()),
            _ => {}
        }
        std::thread::sleep(POLL);
    }
}

/// Puts `before` back in front. A process that is not in front may not do
/// that on its own, so this thread joins the input queue of the window that
/// took the foreground for the call (which needs a queue of its own first).
fn give_back(before: HWND, taken_by: HWND) {
    let own = unsafe { GetCurrentThreadId() };
    let theirs = unsafe { GetWindowThreadProcessId(taken_by, std::ptr::null_mut()) };
    let mut message: MSG = unsafe { std::mem::zeroed() };
    unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE) };
    let attached =
        theirs != 0 && theirs != own && unsafe { AttachThreadInput(own, theirs, 1) } != 0;
    unsafe {
        BringWindowToTop(before);
        SetForegroundWindow(before);
    }
    if attached {
        unsafe { AttachThreadInput(own, theirs, 0) };
    }
}
