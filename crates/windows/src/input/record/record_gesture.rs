//! Folds raw pointer transitions into the gestures a recording reports:
//! the position of a click inside a multi-click, and runs of wheel notches.

use std::time::{Duration, Instant};

/// The system's double-click rule: the same button again inside `window`
/// and within `slop` pixels of the previous press.
#[derive(Debug, Clone, Copy)]
pub(super) struct ClickRule {
    pub(super) window: Duration,
    pub(super) slop_x: i32,
    pub(super) slop_y: i32,
}

/// The double-click rule the user's system is configured with.
#[cfg(target_os = "windows")]
pub(super) fn system_click_rule() -> ClickRule {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXDOUBLECLK, SM_CYDOUBLECLK,
    };
    let (window_ms, slop_x, slop_y) = unsafe {
        (
            GetDoubleClickTime(),
            GetSystemMetrics(SM_CXDOUBLECLK),
            GetSystemMetrics(SM_CYDOUBLECLK),
        )
    };
    ClickRule {
        window: Duration::from_millis(u64::from(window_ms)),
        slop_x: (slop_x / 2).max(1),
        slop_y: (slop_y / 2).max(1),
    }
}

#[derive(Debug, Clone, Copy)]
struct LastClick {
    button: &'static str,
    x: i32,
    y: i32,
    at: Instant,
    count: u32,
}

/// Counts presses into multi-clicks: a double-click is reported as a click
/// with `count` 1 followed by one with `count` 2.
#[derive(Debug)]
pub(super) struct ClickCounter {
    rule: ClickRule,
    last: Option<LastClick>,
}

impl ClickCounter {
    pub(super) fn new(rule: ClickRule) -> Self {
        Self { rule, last: None }
    }

    pub(super) fn register(&mut self, button: &'static str, x: i32, y: i32, at: Instant) -> u32 {
        let count = match self.last {
            Some(last)
                if last.button == button
                    && at.saturating_duration_since(last.at) <= self.rule.window
                    && (x - last.x).abs() <= self.rule.slop_x
                    && (y - last.y).abs() <= self.rule.slop_y =>
            {
                (last.count + 1).min(3)
            }
            _ => 1,
        };
        self.last = Some(LastClick {
            button,
            x,
            y,
            at,
            count,
        });
        count
    }
}

/// One run of wheel notches in a single direction at one place, with the
/// evidence read when the run started.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ScrollRun<T> {
    pub(super) direction: &'static str,
    pub(super) notches: u32,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) started: Instant,
    pub(super) last: Instant,
    pub(super) evidence: T,
}

/// Joins wheel notches into runs. A notch continues the open run when it
/// scrolls the same way, near the same point, within `gap` of the last one.
#[derive(Debug)]
pub(super) struct ScrollAccumulator<T> {
    gap: Duration,
    slop: i32,
    run: Option<ScrollRun<T>>,
}

impl<T> ScrollAccumulator<T> {
    pub(super) fn new(gap: Duration, slop: i32) -> Self {
        Self {
            gap,
            slop,
            run: None,
        }
    }

    /// Whether a notch would continue the open run.
    pub(super) fn continues(&self, direction: &str, x: i32, y: i32, at: Instant) -> bool {
        self.run.as_ref().is_some_and(|run| {
            run.direction == direction
                && at.saturating_duration_since(run.last) <= self.gap
                && (x - run.x).abs() <= self.slop
                && (y - run.y).abs() <= self.slop
        })
    }

    /// Adds a wheel event to the open run. Returns `false` when there is no
    /// run it continues; the caller then closes the old run with `take` and
    /// opens a new one with `open`.
    pub(super) fn extend(&mut self, step: WheelStep, x: i32, y: i32, at: Instant) -> bool {
        if !self.continues(step.direction, x, y, at) {
            return false;
        }
        if let Some(run) = self.run.as_mut() {
            run.notches += step.notches;
            run.last = at;
        }
        true
    }

    pub(super) fn open(&mut self, step: WheelStep, point: (i32, i32), at: Instant, evidence: T) {
        let (x, y) = point;
        self.run = Some(ScrollRun {
            direction: step.direction,
            notches: step.notches,
            x,
            y,
            started: at,
            last: at,
            evidence,
        });
    }

    pub(super) fn take(&mut self) -> Option<ScrollRun<T>> {
        self.run.take()
    }

    /// Closes the open run once no notch has continued it for `gap`.
    pub(super) fn take_if_idle(&mut self, now: Instant) -> Option<ScrollRun<T>> {
        let idle = self
            .run
            .as_ref()
            .is_some_and(|run| now.saturating_duration_since(run.last) > self.gap);
        if idle { self.run.take() } else { None }
    }
}

const WHEEL_DELTA: u32 = 120;

/// One wheel event: its direction and how many notches it carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WheelStep {
    pub(super) direction: &'static str,
    pub(super) notches: u32,
}

/// The step of one wheel event: vertical positive deltas scroll up,
/// horizontal positive deltas scroll right, and every `WHEEL_DELTA` is one
/// notch. A high-resolution wheel's fraction of a notch still counts as one.
pub(super) fn wheel_step(delta: i16, horizontal: bool) -> Option<WheelStep> {
    let direction = match (horizontal, delta.signum()) {
        (false, 1) => "up",
        (false, -1) => "down",
        (true, 1) => "right",
        (true, -1) => "left",
        _ => return None,
    };
    let notches = (u32::from(delta.unsigned_abs()) / WHEEL_DELTA).max(1);
    Some(WheelStep { direction, notches })
}

#[cfg(test)]
#[path = "record_gesture_tests.rs"]
mod tests;
