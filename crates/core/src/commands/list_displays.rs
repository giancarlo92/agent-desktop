use crate::{AppError, adapter::PlatformAdapter};
use serde_json::Value;

pub fn execute(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let displays = adapter.list_displays(crate::Deadline::standard()?)?;
    Ok(crate::commands::record_display::numbered(&displays))
}
