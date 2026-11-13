use std::path::PathBuf;

use agent_desktop_core::AppError;
use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrokeArgs {
    #[arg(
        long,
        help = "Points as x,y;x,y;... or a JSON array of [x, y] or [x, y, ms] (a recorded drag's path); requires --headed"
    )]
    pub path: Option<String>,
    #[arg(long, help = "Read the path from this file instead (same formats)")]
    pub path_file: Option<PathBuf>,
    #[arg(
        long,
        help = "Milliseconds between points that carry no time of their own [default: 16]"
    )]
    pub step_ms: Option<u64>,
    #[arg(
        long,
        help = "Add this to every x and y (a recorded path replayed in a window that moved); write a negative value with = (--offset=-10,5)"
    )]
    pub offset: Option<String>,
}

const DEFAULT_STEP_MS: u64 = 16;

/// The stroke's points as `(x, y, ms)`, `ms` counted from the press.
pub(crate) fn parse_points(args: &StrokeArgs) -> Result<Vec<(f64, f64, u64)>, AppError> {
    let text = match (&args.path, &args.path_file) {
        (Some(path), None) => path.clone(),
        (None, Some(file)) => std::fs::read_to_string(file).map_err(|error| {
            AppError::invalid_input(format!("Cannot read {}: {error}", file.display()))
        })?,
        _ => return Err(AppError::invalid_input("Provide --path or --path-file")),
    };
    let step = args.step_ms.unwrap_or(DEFAULT_STEP_MS);
    let (dx, dy) = match &args.offset {
        Some(offset) => crate::dispatch::parse::parse_xy(offset)?,
        None => (0.0, 0.0),
    };
    let raw = if text.trim_start().starts_with('[') {
        json_points(&text)?
    } else {
        pair_points(&text)?
    };
    let mut points = Vec::with_capacity(raw.len());
    for (index, (x, y, ms)) in raw.into_iter().enumerate() {
        let at = ms.unwrap_or(step.saturating_mul(index as u64));
        points.push((x + dx, y + dy, at));
    }
    Ok(points)
}

fn json_points(text: &str) -> Result<Vec<(f64, f64, Option<u64>)>, AppError> {
    let values: Vec<Vec<f64>> = serde_json::from_str(text).map_err(|error| {
        AppError::invalid_input(format!("The path is not a JSON array of points: {error}"))
    })?;
    values
        .into_iter()
        .map(|point| match point.as_slice() {
            [x, y] => Ok((*x, *y, None)),
            [x, y, ms] if *ms >= 0.0 => Ok((*x, *y, Some(*ms as u64))),
            _ => Err(AppError::invalid_input(
                "Each point is [x, y] or [x, y, ms]",
            )),
        })
        .collect()
}

fn pair_points(text: &str) -> Result<Vec<(f64, f64, Option<u64>)>, AppError> {
    text.split(';')
        .map(str::trim)
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (x, y) = crate::dispatch::parse::parse_xy(pair)?;
            Ok((x, y, None))
        })
        .collect()
}

#[cfg(test)]
#[path = "stroke_tests.rs"]
mod tests;
