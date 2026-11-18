// Part of the desktop-recorder-mcp fork of agent-desktop (see NOTICE).
//! `menu`: run a command of a window's menu bar by its path, in the
//! background. A menu bar only opens in the window in front, so instead of
//! opening it the command is sent to the window the way choosing the item
//! does.

use crate::{AppError, adapter::PlatformAdapter};
use serde_json::{Value, json};

pub struct MenuArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub path: Vec<String>,
}

pub fn execute(args: MenuArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let path: Vec<String> = args
        .path
        .iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect();
    if path.is_empty() {
        return Err(AppError::invalid_input_with_suggestion(
            "The menu path is empty",
            "Name the items from the menu bar down: menu File Export",
        ));
    }
    let window = super::window_target::resolve_window_for_app(
        args.app.as_deref(),
        args.window_id.as_deref(),
        adapter,
    )?;
    let lease = adapter.acquire_interaction_lease(crate::Deadline::standard()?)?;
    let chosen = adapter.invoke_menu(&window, &path, &lease)?;
    Ok(json!({ "window": window.id, "path": chosen, "sent": true }))
}
