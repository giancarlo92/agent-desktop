use serde::Serialize;

/// How a `record` session ended.
///
/// `stop_reason` is `interrupted`, `stop_file` or `max_duration`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordSummary {
    pub duration_ms: u64,
    pub stop_reason: &'static str,
}
