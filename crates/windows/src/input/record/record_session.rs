//! The `record` loop: turns raw transitions into `click`, `drag`, `type`,
//! `press` and `scroll` events.
//!
//! Typing is folded into one `type` event per field. Its `value` is the
//! field's content read once typing pauses, which is what a replay sets; the
//! keystroke text is kept beside it as `typed` while it is exact. A commit key
//! (Return, Tab, Escape, a function key) or a click elsewhere ends the run.

use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use agent_desktop_core::commands::record::RECORDING_READY_KIND;
use agent_desktop_core::{AdapterError, RecordRequest, RecordSink, RecordSummary, RecordedEvent};
use serde_json::{Map, Value, json};

use super::record_copy::{copied, is_copy};
use super::record_element::{FieldValue, Observer, Target};
use super::record_followup::{CLICK_SETTLE, ChoiceWatch, PendingClick, is_container};
use super::record_gesture::{
    ClickCounter, DragTracker, ScrollAccumulator, ScrollRun, system_click_rule, system_drag_slop,
    wheel_step,
};
use super::record_hook::{self, RawInput};
use super::record_keys::{KeyClass, ModifierState, classify, key_name};
use super::record_stop::{InterruptGuard, StopWatch};
use super::record_text::typed_text;

const POLL: Duration = Duration::from_millis(50);
const SETTLE: Duration = Duration::from_millis(40);
const SCROLL_GAP: Duration = Duration::from_millis(600);
const SCROLL_SLOP: i32 = 40;
const VK_BACK: u32 = 0x08;

pub(super) fn run(
    request: &RecordRequest,
    sink: &mut RecordSink<'_>,
) -> Result<RecordSummary, AdapterError> {
    let observer = Observer::new(request)?;
    let _interrupt = InterruptGuard::install();
    let started = Instant::now();
    let mut stop = StopWatch::new(started, request.max_duration_ms, request.stop_file.clone());
    let (hooks, inputs) = record_hook::start(request.include_injected)?;
    let mut recorder = Recorder::new(observer, started, sink);
    recorder.emit(RECORDING_READY_KIND, Instant::now(), Map::new())?;
    let stop_reason = loop {
        if let Some(reason) = stop.finished(Instant::now()) {
            break reason;
        }
        match inputs.recv_timeout(POLL) {
            Ok(input) => recorder.handle(input)?,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(record_hook::hooks_lost()),
        }
        recorder.tick(Instant::now())?;
    };
    drop(hooks);
    while let Ok(input) = inputs.try_recv() {
        recorder.handle(input)?;
    }
    recorder.finish()?;
    Ok(RecordSummary {
        duration_ms: millis_between(started, Instant::now()),
        stop_reason,
    })
}

struct PendingText {
    target: Target,
    /// The field as it was when typing started: a dialog that closes on the
    /// commit key (a file name box) no longer answers when the text is flushed.
    evidence: Map<String, Value>,
    typed: String,
    exact: bool,
    started: Instant,
    last_key: Instant,
    settled: Option<FieldValue>,
}

struct Recorder<'s, 'a> {
    observer: Observer,
    started: Instant,
    sink: &'s mut RecordSink<'a>,
    clicks: ClickCounter,
    drags: DragTracker,
    scroll: ScrollAccumulator<Map<String, Value>>,
    modifiers: ModifierState,
    pending: Option<PendingText>,
    click: Option<PendingClick>,
    choice: Option<ChoiceWatch>,
}

impl<'s, 'a> Recorder<'s, 'a> {
    fn new(observer: Observer, started: Instant, sink: &'s mut RecordSink<'a>) -> Self {
        Self {
            observer,
            started,
            sink,
            clicks: ClickCounter::new(system_click_rule()),
            drags: DragTracker::new(system_drag_slop()),
            scroll: ScrollAccumulator::new(SCROLL_GAP, SCROLL_SLOP),
            modifiers: ModifierState::default(),
            pending: None,
            click: None,
            choice: None,
        }
    }

    fn handle(&mut self, input: RawInput) -> Result<(), AdapterError> {
        if let RawInput::Move { x, y, at } = input {
            self.drags.motion(x, y, at);
            return Ok(());
        }
        self.settle_click()?;
        self.poll_choice(Instant::now())?;
        match input {
            RawInput::Button { button, x, y, at } => {
                tracing::debug!(button, x, y, "record: pointer press");
                self.on_button(button, x, y, at)
            }
            RawInput::Release { x, y, at } => self.on_release(x, y, at),
            RawInput::Move { .. } => Ok(()),
            RawInput::Wheel {
                delta,
                horizontal,
                x,
                y,
                at,
            } => self.on_wheel(delta, horizontal, (x, y), at),
            RawInput::Key {
                vk,
                scan,
                down,
                at,
                clipboard,
            } => self.on_key(vk, scan, down, at, clipboard),
        }
    }

    fn tick(&mut self, now: Instant) -> Result<(), AdapterError> {
        if self
            .click
            .as_ref()
            .is_some_and(|click| now.saturating_duration_since(click.at) >= CLICK_SETTLE)
        {
            self.settle_click()?;
        }
        self.poll_choice(now)?;
        if let Some(run) = self.scroll.take_if_idle(now) {
            self.emit_scroll(run)?;
        }
        if let Some(pending) = self.pending.as_mut() {
            if pending.settled.is_none()
                && now.saturating_duration_since(pending.last_key) >= SETTLE
            {
                if let Some(element) = pending.target.element.as_ref() {
                    pending.settled = Some(self.observer.value(element));
                }
            }
        }
        Ok(())
    }

    fn settle_click(&mut self) -> Result<(), AdapterError> {
        let Some(click) = self.click.take() else {
            return Ok(());
        };
        let at = click.at;
        let (fields, watch) = click.resolve(&mut self.observer);
        self.poll_choice(Instant::now())?;
        let in_popup = self.choice.as_ref().is_some_and(|choice| {
            is_container(&fields) || (fields.contains_key("hit") && choice.targets(&fields))
        });
        if in_popup {
            return Ok(());
        }
        if watch.is_some() {
            self.choice = watch;
        }
        self.emit("click", at, fields)
    }

    fn poll_choice(&mut self, now: Instant) -> Result<(), AdapterError> {
        let Some(mut watch) = self.choice.take() else {
            return Ok(());
        };
        let (changed, keep) = watch.poll(&self.observer, now);
        if keep {
            self.choice = Some(watch);
        }
        match changed {
            Some(fields) => self.emit("select", now, fields),
            None => Ok(()),
        }
    }

    fn finish(&mut self) -> Result<(), AdapterError> {
        self.settle_click()?;
        self.poll_choice(Instant::now())?;
        if let Some(run) = self.scroll.take() {
            self.emit_scroll(run)?;
        }
        self.flush_text()
    }

    fn on_button(
        &mut self,
        button: &'static str,
        x: i32,
        y: i32,
        at: Instant,
    ) -> Result<(), AdapterError> {
        if let Some(run) = self.scroll.take() {
            self.emit_scroll(run)?;
        }
        if button == "left" {
            self.drags.press(x, y, at);
        }
        let target = self.observer.at_point(x, y);
        let count = self.clicks.register(button, x, y, at);
        if self.typing_elsewhere(&target) {
            self.flush_text()?;
        }
        if !self.observer.keeps(&target) {
            tracing::debug!(
                x,
                y,
                listed = target.window.is_some(),
                "record: click outside the filter"
            );
            return Ok(());
        }
        let mut fields = self.observer.describe(&target);
        fields.insert("button".into(), json!(button));
        fields.insert("count".into(), json!(count));
        fields.insert("point".into(), json!({"x": x, "y": y}));
        self.click = Some(PendingClick::new(&mut self.observer, target, fields, at));
        Ok(())
    }

    /// A left release far from its press ends a drag, recorded after the
    /// press's `click` with the element under the release point: the cell a
    /// range selection ended on, or where something was dropped. That
    /// element is read now, so its name and state show what the drag left
    /// (a grid names the range it selected).
    fn on_release(&mut self, x: i32, y: i32, at: Instant) -> Result<(), AdapterError> {
        let Some(drag) = self.drags.release(x, y, at) else {
            return Ok(());
        };
        let (from_x, from_y) = drag.from;
        self.settle_click()?;
        let mut target = self.observer.at_point(x, y);
        for _ in 0..2 {
            if !self.observer.blank(&target) && target.window.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
            target = self.observer.at_point(x, y);
        }
        if !self.observer.keeps(&target) {
            return Ok(());
        }
        let mut fields = self.observer.describe(&target);
        fields.insert("from".into(), json!({"x": from_x, "y": from_y}));
        fields.insert("point".into(), json!({"x": x, "y": y}));
        let path = drag.path.iter().map(|&(px, py, ms)| json!([px, py, ms]));
        fields.insert("path".into(), Value::Array(path.collect()));
        self.emit("drag", at, fields)
    }

    fn on_wheel(
        &mut self,
        delta: i16,
        horizontal: bool,
        point: (i32, i32),
        at: Instant,
    ) -> Result<(), AdapterError> {
        let Some(step) = wheel_step(delta, horizontal) else {
            return Ok(());
        };
        let (x, y) = point;
        if self.scroll.extend(step, x, y, at) {
            return Ok(());
        }
        if let Some(run) = self.scroll.take() {
            self.emit_scroll(run)?;
        }
        let target = self.observer.at_point(x, y);
        if self.observer.keeps(&target) {
            let evidence = self.observer.describe(&target);
            self.scroll.open(step, point, at, evidence);
        }
        Ok(())
    }

    fn on_key(
        &mut self,
        vk: u32,
        scan: u32,
        down: bool,
        at: Instant,
        clipboard: u32,
    ) -> Result<(), AdapterError> {
        let class = classify(vk);
        if let KeyClass::Modifier(key) = class {
            self.modifiers.apply(key, down);
            return Ok(());
        }
        if !down {
            return Ok(());
        }
        if self.modifiers.is_shortcut() {
            self.flush_text()?;
            return self.emit_press(vk, at, clipboard);
        }
        match class {
            KeyClass::Commit => {
                self.flush_text()?;
                self.emit_press(vk, at, clipboard)
            }
            KeyClass::Navigation if self.pending.is_none() => self.emit_press(vk, at, clipboard),
            _ => self.on_typing(vk, scan, class, at),
        }
    }

    fn on_typing(
        &mut self,
        vk: u32,
        scan: u32,
        class: KeyClass,
        at: Instant,
    ) -> Result<(), AdapterError> {
        let target = self.observer.focused();
        if !self.observer.keeps(&target) {
            return self.flush_text();
        }
        if self.typing_elsewhere(&target) {
            self.flush_text()?;
        }
        let text = match class {
            KeyClass::Text => typed_text(vk, scan, &self.modifiers),
            _ => None,
        };
        if self.pending.is_none() {
            let evidence = self.observer.describe(&target);
            self.pending = Some(PendingText {
                target,
                evidence,
                typed: String::new(),
                exact: true,
                started: at,
                last_key: at,
                settled: None,
            });
        }
        let Some(pending) = self.pending.as_mut() else {
            return Ok(());
        };
        match (class, text) {
            (KeyClass::Text, Some(text)) => pending.typed.push_str(&text),
            (KeyClass::Text, None) => {}
            (KeyClass::Editing, _) if vk == VK_BACK && pending.typed.pop().is_some() => {}
            _ => pending.exact = false,
        }
        pending.last_key = at;
        pending.settled = None;
        Ok(())
    }

    fn typing_elsewhere(&self, target: &Target) -> bool {
        let Some(pending) = self.pending.as_ref() else {
            return false;
        };
        match (pending.target.element.as_ref(), target.element.as_ref()) {
            (Some(typing), Some(next)) => !self.observer.same(typing, next),
            _ => true,
        }
    }

    fn flush_text(&mut self) -> Result<(), AdapterError> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        let value = match pending.settled {
            Some(value) => value,
            None => pending
                .target
                .element
                .as_ref()
                .map_or(FieldValue::Unavailable, |element| {
                    self.observer.value(element)
                }),
        };
        let mut fields = pending.evidence;
        match value {
            FieldValue::Secret => {
                fields.insert("secure".into(), json!(true));
            }
            FieldValue::Text(text) => {
                fields.insert("value".into(), json!(text));
                fields.insert("source".into(), json!("value"));
                if pending.exact && !pending.typed.is_empty() {
                    fields.insert("typed".into(), json!(pending.typed));
                }
            }
            FieldValue::Unavailable if pending.typed.is_empty() => return Ok(()),
            FieldValue::Unavailable => {
                fields.insert("typed".into(), json!(pending.typed));
                fields.insert("source".into(), json!("keys"));
            }
        }
        self.emit("type", pending.started, fields)
    }

    /// A key press. A copy (ctrl+c, ctrl+x) also records the shape of what it
    /// put on the clipboard, `clipboard` being the sequence number before it.
    fn emit_press(&mut self, vk: u32, at: Instant, clipboard: u32) -> Result<(), AdapterError> {
        let target = self.observer.focused();
        if !self.observer.keeps(&target) {
            return Ok(());
        }
        let mut fields = self.observer.describe(&target);
        match key_name(vk) {
            Some(name) => {
                let keys = self.modifiers.combo(&name);
                if is_copy(&keys) {
                    if let Some(shape) = copied(clipboard) {
                        fields.insert("clipboard".into(), shape);
                    }
                }
                fields.insert("keys".into(), json!(keys))
            }
            None => fields.insert("vk".into(), json!(vk)),
        };
        self.emit("press", at, fields)
    }

    fn emit_scroll(&mut self, run: ScrollRun<Map<String, Value>>) -> Result<(), AdapterError> {
        let mut fields = run.evidence;
        fields.insert("direction".into(), json!(run.direction));
        fields.insert("amount".into(), json!(run.notches));
        fields.insert("point".into(), json!({"x": run.x, "y": run.y}));
        self.emit("scroll", run.started, fields)
    }

    fn emit(
        &mut self,
        kind: &'static str,
        at: Instant,
        fields: Map<String, Value>,
    ) -> Result<(), AdapterError> {
        (self.sink)(RecordedEvent {
            kind,
            elapsed_ms: millis_between(self.started, at),
            fields,
        })
    }
}

fn millis_between(start: Instant, at: Instant) -> u64 {
    u64::try_from(at.saturating_duration_since(start).as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "record_session_tests.rs"]
mod tests;
