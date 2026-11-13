//! Browser windows.
//!
//! A recorded web action needs the page it happened on, which only the
//! address bar shows, so the recorder reads it into the window's `url`.
//! Chromium also builds the accessibility tree of a page only once a client
//! asks for it: the first hit test on a fresh page returns the bare browser
//! widget, and a moment later the element under the pointer.

use std::time::Duration;

use uiautomation::UIAutomation;
use uiautomation::types::{Handle, TreeScope, UIProperty};
use uiautomation::variants::Variant;

use crate::tree::element::UIAElement;
use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::properties::{PropertyOutcome, PropertyValue, read_one};

const BROWSERS: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "brave.exe",
    "vivaldi.exe",
    "opera.exe",
];

const ADDRESS_BAR: &str = "Address and search bar";

const CHROMIUM_WIDGET: &str = "Chrome_WidgetWin_1";

/// How many times a hit on the bare Chromium widget is read again, and how
/// long apart, while the page builds its accessibility tree.
pub(super) const CHROMIUM_RETRIES: usize = 4;
pub(super) const CHROMIUM_RETRY_GAP: Duration = Duration::from_millis(150);

/// Whether an application is a browser whose address the recorder reads.
pub(super) fn is_browser(app: &str) -> bool {
    BROWSERS.iter().any(|name| app.eq_ignore_ascii_case(name))
}

/// Whether a hit is Chromium's own unnamed widget instead of an element of
/// the page.
pub(super) fn is_bare_chromium(class: Option<&str>, named: bool) -> bool {
    !named && class == Some(CHROMIUM_WIDGET)
}

/// The address bar's text in the browser window `root`, if it has one.
pub(super) fn address(client: &UIAutomation, root: isize) -> Option<String> {
    let window = client.element_from_handle(Handle::from(root)).ok()?;
    let condition = client
        .create_property_condition(UIProperty::Name, Variant::from(ADDRESS_BAR), None)
        .ok()?;
    let bar = window.find_first(TreeScope::Descendants, &condition).ok()?;
    match read_one(&UIAElement::from(bar), TreeProperty::Value) {
        PropertyOutcome::Known(PropertyValue::Text(text)) if !text.trim().is_empty() => {
            Some(text.trim().to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "record_browser_tests.rs"]
mod tests;
