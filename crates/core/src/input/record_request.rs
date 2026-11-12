use std::path::PathBuf;

/// What a `record` session watches and when it stops.
///
/// `app` and `window_id` narrow which windows' events are kept; both absent
/// keeps every window except the recorder's own process. A session ends at
/// the first of: the caller's interrupt, `stop_file` appearing on disk, or
/// `max_duration_ms` elapsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRequest {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub max_duration_ms: u64,
    pub stop_file: Option<PathBuf>,
    pub include_injected: bool,
}
