use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub(crate) struct RecordArgs {
    #[arg(long, help = "JSON Lines file the recording is streamed to")]
    pub out: PathBuf,
    #[arg(long, help = "Keep only actions in this application's windows")]
    pub app: Option<String>,
    #[arg(
        long,
        help = "Keep only actions in this window (an id from list-windows)"
    )]
    pub window_id: Option<String>,
    #[arg(
        long,
        help = "Keep only actions on this display (an id from list-displays)"
    )]
    pub display: Option<String>,
    #[arg(
        long,
        help = "Stop after this many milliseconds (default 600000, at most 3600000)"
    )]
    pub max_duration_ms: Option<u64>,
    #[arg(long, help = "Stop when this file appears; it must not exist yet")]
    pub stop_file: Option<PathBuf>,
    #[arg(
        long,
        help = "Also record input Windows marks as injected (synthesized by software)"
    )]
    pub include_injected: bool,
}
