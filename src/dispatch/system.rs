use agent_desktop_core::{
    AppError, PermissionReport, PlatformAdapter,
    commands::{
        permissions as permissions_command, record as record_command, skills as skills_command,
        wait as wait_command, wait_surface::SurfaceWait,
    },
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::{
    record::RecordArgs,
    skills::{SkillsAction, SkillsArgs},
    system::{PermissionsArgs, WaitArgs},
};

pub(super) fn wait(
    args: WaitArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    wait_command::execute(
        wait_command::WaitArgs {
            mode: wait_command::WaitModeArgs {
                ms: args.mode.ms,
                element: args.mode.element,
                window: args.mode.window,
                text: args.mode.text,
                surface: SurfaceWait::from_flags(
                    args.mode.menu,
                    args.mode.menu_closed,
                    args.mode.notification,
                )?,
                event: args.event.event,
                window_id: args.event.window_id,
            },
            predicate: wait_command::WaitPredicateArgs {
                predicate: args.predicate.predicate,
                value: args.predicate.value,
                action: args.predicate.action,
                count: args.predicate.count,
            },
            timeout_ms: args.timeout,
            app: args.app,
        },
        adapter,
        context,
    )
}

pub(super) fn permissions(
    args: PermissionsArgs,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
) -> Result<Value, AppError> {
    permissions_command::execute_with_report(
        permissions_command::PermissionsArgs {
            request: args.request,
            guide: args.guide,
        },
        adapter,
        permission_report,
    )
}

pub(super) fn record(args: RecordArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    use crate::cli_args::record::RecordAction;
    use agent_desktop_core::commands::record_background;
    match args.action {
        Some(RecordAction::Start(start)) => {
            record_background::start(record_background::StartArgs {
                out: start.out,
                app: start.scope.app,
                window_id: start.scope.window_id,
                display: start.scope.display,
                max_duration_ms: start.scope.max_duration_ms,
                include_injected: start.scope.include_injected,
                video: start.video.video,
                video_out: start.video.video_out,
            })
        }
        Some(RecordAction::Stop(stop)) => {
            record_background::stop(stop.id.as_deref(), stop.timeout_ms)
        }
        None => {
            let out = args.out.ok_or_else(|| {
                AppError::invalid_input_with_suggestion(
                    "record needs --out",
                    "Pass --out FILE, or use 'record start' to record in the background",
                )
            })?;
            let video = match (args.video.video_out, args.video.video) {
                (Some(path), _) => Some(path),
                (None, true) => Some(record_command::default_video_path(&out)),
                (None, false) => None,
            };
            record_command::execute(
                record_command::RecordArgs {
                    out,
                    app: args.scope.app,
                    window_id: args.scope.window_id,
                    display: args.scope.display,
                    max_duration_ms: args.scope.max_duration_ms,
                    stop_file: args.stop_file,
                    include_injected: args.scope.include_injected,
                    video,
                },
                adapter,
            )
        }
    }
}

pub(super) fn skills(args: SkillsArgs) -> Result<Value, AppError> {
    match args.action.unwrap_or(SkillsAction::List) {
        SkillsAction::List => skills_command::list(),
        SkillsAction::Path => skills_command::path(),
        SkillsAction::Get(get) => skills_command::get(skills_command::GetArgs {
            name: get.name,
            full: get.full,
            reference: get.reference,
        }),
    }
}
