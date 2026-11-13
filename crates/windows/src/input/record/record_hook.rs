//! The low-level hook thread.
//!
//! Each callback copies the transition into a channel and returns at once:
//! Windows silently removes a low-level hook whose callback exceeds the
//! system's hook timeout, so nothing here reads UI Automation or blocks.

use std::cell::{Cell, RefCell};
use std::sync::mpsc::{Receiver, Sender, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::Instant;

use agent_desktop_core::{AdapterError, ErrorCode};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW,
    PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL,
    WM_QUIT,
};

const HC_ACTION: i32 = 0;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_MOUSEHWHEEL: u32 = 0x020E;
const LLMHF_INJECTED: u32 = 0x01;
const LLMHF_LOWER_IL_INJECTED: u32 = 0x02;
const LLKHF_LOWER_IL_INJECTED: u32 = 0x02;
const LLKHF_INJECTED: u32 = 0x10;

/// One raw transition the hooks observed.
#[derive(Debug, Clone, Copy)]
pub(super) enum RawInput {
    Button {
        button: &'static str,
        x: i32,
        y: i32,
        at: Instant,
    },
    /// The left button went up: with its press, it tells a drag from a click.
    Release { x: i32, y: i32, at: Instant },
    Wheel {
        delta: i16,
        horizontal: bool,
        x: i32,
        y: i32,
        at: Instant,
    },
    Key {
        vk: u32,
        scan: u32,
        down: bool,
        at: Instant,
        /// The clipboard's sequence number when the key was pressed, read
        /// before the application handles the key: a copy shortcut changes it.
        clipboard: u32,
    },
}

thread_local! {
    static SENDER: RefCell<Option<Sender<RawInput>>> = const { RefCell::new(None) };
    static KEEP_INJECTED: Cell<bool> = const { Cell::new(false) };
}

fn forward(input: RawInput) {
    SENDER.with(|sender| {
        if let Some(sender) = sender.borrow().as_ref() {
            let _ = sender.send(input);
        }
    });
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION && lparam != 0 {
        let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
        let injected = info.flags & (LLMHF_INJECTED | LLMHF_LOWER_IL_INJECTED) != 0;
        if !injected || KEEP_INJECTED.with(Cell::get) {
            if let Some(input) = mouse_input(wparam as u32, info) {
                forward(input);
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

fn mouse_input(message: u32, info: &MSLLHOOKSTRUCT) -> Option<RawInput> {
    let (x, y, at) = (info.pt.x, info.pt.y, Instant::now());
    let button = match message {
        WM_LBUTTONDOWN => "left",
        WM_RBUTTONDOWN => "right",
        WM_MBUTTONDOWN => "middle",
        WM_LBUTTONUP => return Some(RawInput::Release { x, y, at }),
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            return Some(RawInput::Wheel {
                delta: (info.mouseData >> 16) as u16 as i16,
                horizontal: message == WM_MOUSEHWHEEL,
                x,
                y,
                at,
            });
        }
        _ => return None,
    };
    Some(RawInput::Button { button, x, y, at })
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION && lparam != 0 {
        let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let injected = info.flags & (LLKHF_INJECTED | LLKHF_LOWER_IL_INJECTED) != 0;
        let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        let up = matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP);
        if (down || up) && (!injected || KEEP_INJECTED.with(Cell::get)) {
            forward(RawInput::Key {
                vk: info.vkCode,
                scan: info.scanCode,
                down,
                at: Instant::now(),
                clipboard: unsafe { GetClipboardSequenceNumber() },
            });
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// The running hook thread. Dropping it unhooks and joins the thread.
pub(super) struct HookThread {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl Drop for HookThread {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Starts the hook thread and returns once both hooks are installed.
pub(super) fn start(keep_injected: bool) -> Result<(HookThread, Receiver<RawInput>), AdapterError> {
    let (events, received) = channel();
    let (ready_tx, ready_rx) = sync_channel::<Result<u32, u32>>(1);
    let join = std::thread::Builder::new()
        .name("agent-desktop-record-hooks".into())
        .spawn(move || run_hooks(events, keep_injected, ready_tx))
        .map_err(|error| hook_error(format!("start the input hook thread: {error}")))?;
    match ready_rx.recv() {
        Ok(Ok(thread_id)) => Ok((
            HookThread {
                thread_id,
                join: Some(join),
            },
            received,
        )),
        Ok(Err(code)) => {
            let _ = join.join();
            Err(hook_error(format!(
                "install the low-level input hooks (Win32 error {code})"
            )))
        }
        Err(_) => Err(hook_error(
            "the input hook thread ended before it was ready",
        )),
    }
}

fn run_hooks(
    events: Sender<RawInput>,
    keep_injected: bool,
    ready: std::sync::mpsc::SyncSender<Result<u32, u32>>,
) {
    SENDER.with(|sender| *sender.borrow_mut() = Some(events));
    KEEP_INJECTED.with(|keep| keep.set(keep_injected));
    let module = unsafe { GetModuleHandleW(std::ptr::null()) };
    let mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0) };
    let keyboard = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0) };
    if mouse.is_null() || keyboard.is_null() {
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        unhook(mouse);
        unhook(keyboard);
        let _ = ready.send(Err(code));
        return;
    }
    let mut message: MSG = unsafe { std::mem::zeroed() };
    unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE) };
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
    while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {}
    unhook(mouse);
    unhook(keyboard);
    SENDER.with(|sender| *sender.borrow_mut() = None);
}

fn unhook(hook: windows_sys::Win32::UI::WindowsAndMessaging::HHOOK) {
    if !hook.is_null() {
        unsafe { UnhookWindowsHookEx(hook) };
    }
}

/// The error a session ends with when the hook thread stops before it does.
pub(super) fn hooks_lost() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The input hooks stopped before the recording session ended",
    )
}

fn hook_error(message: impl Into<String>) -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, message)
        .with_suggestion("Run record from an interactive desktop session")
}
