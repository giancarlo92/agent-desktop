//! This build always works in the background: it never uses the person's
//! mouse or keyboard and never brings a window to the front, so the person
//! can keep working while a script or an agent runs. There is no switch to
//! turn it off.
//!
//! `--headed` is refused, and so is every command that only works with
//! physical input or by taking the foreground. Everything else uses UI
//! Automation and window messages (see the Windows adapter's
//! `background_value`, `background_keyboard` and `focus_guard`).

use agent_desktop_core::{AdapterError, AppError, DeliverySemantics, ErrorCode};

use crate::cli::Commands;

const WHY: &str = "This build of agent-desktop works only in the background: it never uses the \
person's mouse or keyboard and never brings a window to the front";

/// The commands refused always, with what to use instead.
pub(crate) const REFUSED: &[(&str, &str)] = &[
    (
        "press",
        "To submit, click the button; to enter text, use set-value",
    ),
    (
        "key-down",
        "Hold no keys: use click, set-value or type on an element",
    ),
    (
        "key-up",
        "Hold no keys: use click, set-value or type on an element",
    ),
    (
        "focus",
        "Act on the element directly: click, set-value and type need no focus",
    ),
    (
        "focus-window",
        "Act on the window's elements directly: a window behind others is drivable",
    ),
    (
        "double-click",
        "Use the element's action instead (click, expand, select)",
    ),
    (
        "triple-click",
        "Use set-value or clear on the field instead",
    ),
    ("right-click", "Use the element's own actions instead"),
    ("hover", "Read the element instead (get, find)"),
    (
        "drag",
        "Use the element's actions (select, set-value) instead",
    ),
    (
        "stroke",
        "Drawing needs the mouse; it is not available in the background",
    ),
    ("mouse-click", "Click the element by its ref instead"),
    ("mouse-move", "Act on elements by ref instead"),
    ("mouse-down", "Act on elements by ref instead"),
    ("mouse-up", "Act on elements by ref instead"),
    (
        "mouse-wheel",
        "Use scroll or scroll-to on the element instead",
    ),
    (
        "maximize",
        "Use resize-window instead: maximizing brings the window to the front",
    ),
];

pub(crate) fn headed_refused() -> AppError {
    refusal_error(
        "--headed is not available",
        "Run the command without --headed: it works in the background",
    )
}

/// The refusal for a command this build never runs, if `cmd` is one.
pub(crate) fn refusal(cmd: &Commands) -> Option<AppError> {
    if let Commands::Launch(args) = cmd
        && args.activate
    {
        return Some(refusal_error(
            "launch --activate is not available",
            "Launch without --activate: the app opens behind the person's windows",
        ));
    }
    let name = cmd.name();
    // On Windows `type` sends keystrokes; set-value writes the same text in
    // the background.
    if cfg!(target_os = "windows") && name == "type" {
        return Some(refusal_error(
            "type is not available",
            "Use set-value with the whole text instead",
        ));
    }
    REFUSED
        .iter()
        .find(|(refused, _)| *refused == name)
        .map(|(refused, instead)| refusal_error(&format!("{refused} is not available"), instead))
}

fn refusal_error(what: &str, instead: &str) -> AppError {
    AppError::Adapter(
        AdapterError::new(ErrorCode::PolicyDenied, format!("{what}. {WHY}."))
            .with_suggestion(instead.to_string())
            .with_disposition(DeliverySemantics::not_delivered()),
    )
}

#[cfg(test)]
#[path = "background_only_tests.rs"]
mod tests;
