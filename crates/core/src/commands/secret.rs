//! `secret`: passwords a person stores once and scripts and agents use by
//! name, without ever reading them.
//!
//! `set` saves a value the person typed (the CLI asks for it without showing
//! it); `list` shows names and bindings; `delete` removes one; `fill` writes a
//! secret into a field after checking it may go there (see
//! `secret_policy`), and removes the value from anything it returns. No
//! command returns a value. Through MCP only `list` and `fill` are tools.

use serde_json::{Value, json};

use crate::AppError;
use crate::adapter::PlatformAdapter;
use crate::commands::helpers::load_ref_entry;
use crate::commands::secret_policy::{FieldFacts, StoredSecret, redact, refusal};
use crate::commands::secret_store::{self, SecretInfo};
use crate::context::CommandContext;

const MAX_NAME: usize = 64;
const MAX_SECRET: usize = 1_024;
const ADDRESS_BAR: &str = "Address and search bar";

pub struct SetArgs {
    pub name: String,
    pub value: String,
    pub app: Option<String>,
    pub site: Option<String>,
    pub any_field: bool,
}

pub struct FillArgs {
    pub ref_id: String,
    pub name: String,
    pub timeout_ms: Option<u64>,
}

pub fn valid_name(name: &str) -> Result<(), AppError> {
    let ok = !name.is_empty()
        && name.len() <= MAX_NAME
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(AppError::invalid_input(format!(
            "A secret name has 1 to {MAX_NAME} letters, digits, '-', '_' or '.'"
        )))
    }
}

pub fn set(args: SetArgs) -> Result<Value, AppError> {
    valid_name(&args.name)?;
    if args.value.is_empty() || args.value.len() > MAX_SECRET {
        return Err(AppError::invalid_input(format!(
            "A secret has 1 to {MAX_SECRET} bytes"
        )));
    }
    let secret = StoredSecret {
        secret: args.value,
        app: args.app.clone(),
        site: args.site.clone(),
        any_field: args.any_field,
    };
    secret_store::save(&args.name, &secret)?;
    let info = SecretInfo {
        name: args.name.clone(),
        app: args.app,
        site: args.site,
        any_field: args.any_field,
        saved_ms: now_ms(),
    };
    let index = secret_store::default_index()?;
    let entries = secret_store::with_entry(secret_store::read_index(&index), info.clone());
    secret_store::write_index(&index, &entries)?;
    Ok(json!({ "saved": info }))
}

pub fn list() -> Result<Value, AppError> {
    let index = secret_store::default_index()?;
    Ok(json!({ "secrets": secret_store::read_index(&index) }))
}

pub fn delete(name: &str) -> Result<Value, AppError> {
    valid_name(name)?;
    let removed = secret_store::remove(name)?;
    let index = secret_store::default_index()?;
    let entries: Vec<SecretInfo> = secret_store::read_index(&index)
        .into_iter()
        .filter(|entry| entry.name != name)
        .collect();
    secret_store::write_index(&index, &entries)?;
    Ok(json!({ "name": name, "removed": removed }))
}

pub fn fill(
    args: FillArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    valid_name(&args.name)?;
    let Some(secret) = secret_store::load(&args.name)? else {
        return Err(AppError::invalid_input_with_suggestion(
            format!("No secret is saved as '{}'", args.name),
            "A person saves it once with: agent-desktop secret set <name>",
        ));
    };
    let entry = load_ref_entry(&args.ref_id, context)?;
    let address = secret
        .site
        .as_ref()
        .and_then(|_| browser_address(&entry, adapter, context));
    let facts = FieldFacts {
        role: &entry.identity.role,
        states: &entry.capabilities.states,
        app: entry.source.source_app.as_deref(),
        address: address.as_deref(),
    };
    if let Some(reason) = refusal(&secret, &facts) {
        return Err(AppError::invalid_input(format!(
            "The secret '{}' was not written: {reason}",
            args.name
        )));
    }
    // Typing it, like a person, fires the events a page listens for (a React
    // or Vue form keeps its own copy of the value); setting it does not.
    // The write goes through type and set-value themselves, so it keeps their
    // input policy, verification and errors.
    let typed = context.is_headed();
    let outcome = if typed {
        crate::commands::type_text::execute(
            crate::commands::type_text::TypeArgs {
                ref_id: args.ref_id,
                text: secret.secret.clone(),
                timeout_ms: args.timeout_ms,
                interval_ms: None,
            },
            adapter,
            context,
        )
    } else {
        crate::commands::set_value::execute(
            crate::commands::set_value::SetValueArgs {
                ref_id: args.ref_id,
                value: secret.secret.clone(),
                timeout_ms: args.timeout_ms,
            },
            adapter,
            context,
        )
    };
    match outcome {
        Ok(mut value) => {
            redact(&mut value, &secret.secret);
            if let Some(fields) = value.as_object_mut() {
                for revealing in ["keystrokes", "text", "typed", "value"] {
                    fields.remove(revealing);
                }
            }
            value["secret"] = json!(args.name);
            value["method"] = json!(if typed { "typed" } else { "set" });
            if !typed {
                value["note"] = json!(
                    "The value was set directly. A page that reacts to typing (a React or Vue \
                     form) may not notice it: run with --headed to type it like a person"
                );
            }
            Ok(value)
        }
        Err(error) => Err(redact_error(error, &secret.secret)),
    }
}

/// The address bar's text in the field's browser window, if it has one.
fn browser_address(
    entry: &crate::RefEntry,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Option<String> {
    use crate::commands::find::{self, FindArgs, FindFilterArgs, FindSelectionArgs};
    let found = find::execute(
        FindArgs {
            app: entry.source.source_app.clone(),
            window_id: entry.source.source_window_id.clone(),
            root: None,
            surface: crate::SnapshotSurface::Window,
            filter: FindFilterArgs {
                role: Some("textfield".into()),
                name: Some(ADDRESS_BAR.into()),
                description: None,
                native_id: None,
                value: None,
                text: None,
                exact: true,
            },
            states: Vec::new(),
            selection: FindSelectionArgs {
                count: false,
                first: true,
                last: false,
                nth: None,
                limit: None,
            },
            timeout_ms: Some(5_000),
        },
        adapter,
        context,
    )
    .ok()?;
    found["match"]["value"]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(String::from)
}

fn redact_error(error: AppError, secret: &str) -> AppError {
    match error {
        AppError::Adapter(mut adapter) => {
            adapter.message = adapter.message.replace(secret, "[secret]");
            if let Some(details) = adapter.details.as_mut() {
                redact(details, secret);
            }
            AppError::Adapter(adapter)
        }
        AppError::Internal(text) => AppError::Internal(text.replace(secret, "[secret]")),
        other => other,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
