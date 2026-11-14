// Modified in the desktop-recorder-mcp fork of agent-desktop (see NOTICE).
use crate::{
    AdapterError, ClipboardContent, ClipboardFormat, Deadline, DragParams, InteractionLease,
    KeyCombo, MouseEvent, OperationReport, RecordRequest, RecordSummary, RecordedEvent,
    StrokeParams,
};

/// Receives each event a `record` session observes, in order. An error stops
/// the session and is returned from `record_input`.
pub type RecordSink<'a> = dyn FnMut(RecordedEvent) -> Result<(), AdapterError> + 'a;

/// `get_clipboard`/`set_clipboard` were removed pre-1.0 in favor of
/// `get_clipboard_content`/`set_clipboard_content`; the C ABI
/// (`ad_get_clipboard`/`ad_set_clipboard`) is unaffected.
pub trait InputOps: Send + Sync {
    /// The reason physical pointer delivery cannot happen in this session, if
    /// it cannot. Commands that would aim a cursor ask before any geometry or
    /// actionability work, so a session without pointer input answers at once
    /// instead of spending its wait budget on a gesture it can never send.
    fn physical_pointer_refusal(&self) -> Option<AdapterError> {
        None
    }

    /// Whether a headed keyboard delivery needs the target window activated
    /// first. An adapter whose keys can be delivered without activating
    /// another window (a Wayland session, where the compositor refuses client
    /// activation) answers false, so core skips a window focus the session
    /// cannot grant.
    fn keyboard_needs_window_activation(&self) -> bool {
        true
    }

    fn mouse_event(
        &self,
        _event: MouseEvent,
        _lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        Err(AdapterError::not_supported("mouse_event"))
    }

    fn key_event(
        &self,
        _combo: &KeyCombo,
        _down: bool,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("key_event"))
    }

    fn drag(
        &self,
        _params: DragParams,
        _lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        Err(AdapterError::not_supported("drag"))
    }

    /// Holds the left button down at the stroke's first point, moves the
    /// pointer through its points at their times and releases it at the last.
    fn stroke(
        &self,
        _params: StrokeParams,
        _lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        Err(AdapterError::not_supported("stroke"))
    }

    fn clear_clipboard(&self, _lease: &InteractionLease) -> Result<OperationReport, AdapterError> {
        Err(AdapterError::not_supported("clear_clipboard"))
    }

    /// Reads the requested clipboard representation. Returns `Ok(None)`
    /// when the pasteboard has no data of the requested shape (or, for
    /// `Auto`, no data at all) — a normal, non-error outcome distinct from
    /// `Err(not_supported)`, which means this platform never implements
    /// clipboard reads.
    fn get_clipboard_content(
        &self,
        _format: ClipboardFormat,
        _deadline: Deadline,
    ) -> Result<Option<ClipboardContent>, AdapterError> {
        Err(AdapterError::not_supported("get_clipboard_content"))
    }

    fn set_clipboard_content(
        &self,
        _content: &ClipboardContent,
        _lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        Err(AdapterError::not_supported("set_clipboard_content"))
    }

    /// Observes the user's own clicks, typing, key presses and scrolls until
    /// the request's stop condition, passing each one to `sink` as it
    /// happens. It never sends input and never takes the interaction lease.
    fn record_input(
        &self,
        _request: &RecordRequest,
        _sink: &mut RecordSink<'_>,
    ) -> Result<RecordSummary, AdapterError> {
        Err(AdapterError::not_supported("record_input"))
    }

    /// Asks the person at the screen for a secret in a window of the
    /// platform's own, its characters hidden. `Ok(None)` when they cancel.
    fn ask_secret(&self, _title: &str, _message: &str) -> Result<Option<String>, AdapterError> {
        Err(AdapterError::not_supported("ask_secret"))
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
