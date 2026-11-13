use clap::{Args, Subcommand};
use std::path::PathBuf;

/// `record` runs in the foreground until stopped; `record start` and
/// `record stop` run the same session in the background between two calls.
#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
pub(crate) struct RecordArgs {
    #[command(subcommand)]
    pub action: Option<RecordAction>,
    #[arg(
        long,
        help = "JSON Lines file the recording is streamed to (required without start/stop)"
    )]
    pub out: Option<PathBuf>,
    #[command(flatten)]
    pub scope: RecordScope,
    #[arg(long, help = "Stop when this file appears; it must not exist yet")]
    pub stop_file: Option<PathBuf>,
    #[command(flatten)]
    pub video: RecordVideo,
}

#[derive(Subcommand, Debug)]
pub(crate) enum RecordAction {
    #[command(
        about = "Start recording in the background and return at once with the session's id and paths"
    )]
    Start(RecordStartArgs),
    #[command(about = "Stop a background recording and return its summary")]
    Stop(RecordStopArgs),
}

#[derive(Args, Debug)]
pub(crate) struct RecordStartArgs {
    #[arg(
        long,
        help = "JSON Lines file the recording is streamed to (default: <state root>/recordings/<id>.jsonl)"
    )]
    pub out: Option<PathBuf>,
    #[command(flatten)]
    pub scope: RecordScope,
    #[command(flatten)]
    pub video: RecordVideo,
}

#[derive(Args, Debug)]
pub(crate) struct RecordStopArgs {
    #[arg(
        long,
        help = "The recording to stop (needed when more than one is running)"
    )]
    pub id: Option<String>,
    #[arg(
        long,
        help = "How long to wait for the session to finish, video included (default 60000)"
    )]
    pub timeout_ms: Option<u64>,
}

#[derive(Args, Debug)]
pub(crate) struct RecordScope {
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
    #[arg(
        long,
        help = "Also record input Windows marks as injected (synthesized by software)"
    )]
    pub include_injected: bool,
}

#[derive(Args, Debug)]
pub(crate) struct RecordVideo {
    #[arg(
        long,
        help = "Also record an MP4 video of the recorded window or display (Windows), beside the events unless --video-out"
    )]
    pub video: bool,
    #[arg(long, help = "Write the video to this .mp4 file (implies --video)")]
    pub video_out: Option<PathBuf>,
}
