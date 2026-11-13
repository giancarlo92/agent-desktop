use crate::{
    AppError, Point, StrokeParams, StrokePoint,
    adapter::PlatformAdapter,
    commands::{
        operation_response::operation_response,
        point_resolve::{require_cursor_policy, require_pointer_capability},
    },
    context::CommandContext,
};
use serde_json::{Value, json};

/// Extra time the stroke may take beyond its own duration: moving to the
/// first point, the pickup pause and the release.
const STROKE_OVERHEAD_MS: u64 = 5_000;

pub struct StrokeArgs {
    /// `(x, y, at_ms)` points; `at_ms` counts from the press.
    pub points: Vec<(f64, f64, u64)>,
}

pub fn execute(
    args: StrokeArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    require_cursor_policy(context, "stroke")?;
    require_pointer_capability(adapter)?;
    let params = StrokeParams {
        points: args
            .points
            .iter()
            .map(|&(x, y, at_ms)| StrokePoint {
                point: Point { x, y },
                at_ms,
            })
            .collect(),
    };
    params.validate()?;
    let budget = params.duration_ms() + STROKE_OVERHEAD_MS;
    let deadline = crate::Deadline::after(budget)?;
    let _cursor = crate::cursor_overlay::hold_cursor(context, deadline);
    let lease = adapter.acquire_interaction_lease(deadline)?;
    let points = params.points.len();
    let duration_ms = params.duration_ms();
    let report = adapter.stroke(params, &lease)?;
    operation_response(
        &report,
        json!({ "points": points, "duration_ms": duration_ms }),
    )
}
