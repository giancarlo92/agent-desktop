use std::io::{BufRead, IsTerminal};

use agent_desktop_core::{
    AppError, PlatformAdapter,
    commands::secret::{self, FillArgs, SetArgs},
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::secret::{SecretAction, SecretArgs, SecretSetArgs};

pub(super) fn run(
    args: SecretArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    match args.action {
        SecretAction::Set(set) => {
            secret::valid_name(&set.name)?;
            let value = read_value(&set)?;
            secret::set(SetArgs {
                name: set.name,
                value,
                app: set.app,
                site: set.site,
                any_field: set.any_field,
            })
        }
        SecretAction::List => secret::list(),
        SecretAction::Delete(name) => secret::delete(&name.name),
        SecretAction::Fill(fill) => secret::fill(
            FillArgs {
                ref_id: fill.ref_id,
                name: fill.name,
                timeout_ms: fill.timeout_ms,
            },
            adapter,
            context,
        ),
    }
}

/// The value, typed by a person at a hidden prompt (twice), or the first line
/// of standard input with --stdin. Never an argument: arguments show up in
/// process lists and shell histories.
fn read_value(set: &SecretSetArgs) -> Result<String, AppError> {
    if set.stdin {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(AppError::Io)?;
        // A PowerShell pipe can start with a byte order mark.
        let line = line.trim_start_matches('\u{feff}');
        return Ok(line.trim_end_matches(['\r', '\n']).to_string());
    }
    if !std::io::stdin().is_terminal() {
        return Err(AppError::invalid_input_with_suggestion(
            "secret set asks a person for the value at a terminal",
            "Run it in a terminal, or pass --stdin to read the value from standard input",
        ));
    }
    let first = rpassword::prompt_password(format!("Value for '{}' (hidden): ", set.name))
        .map_err(AppError::Io)?;
    let again = rpassword::prompt_password("Type it again: ").map_err(AppError::Io)?;
    if first != again {
        return Err(AppError::invalid_input(
            "The two values differ; nothing was saved",
        ));
    }
    Ok(first)
}
