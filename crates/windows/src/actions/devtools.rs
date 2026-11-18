//! Click, set-value and clear on a web page through the browser's DevTools
//! endpoint, when the browser was started with one.
//!
//! Chromium brings its own window to the front when it receives one of these
//! as an accessibility action (see `focus_guard`). Through DevTools it never
//! does, so the person's window is not disturbed even for an instant. The
//! route is taken only when all of this holds, and otherwise the caller goes
//! on to the accessibility action:
//!
//! - the element is Chromium content inside exactly one web document (not
//!   browser chrome, not a nested frame);
//! - the element's process listens on a loopback port that answers as a
//!   DevTools endpoint;
//! - exactly one element of the page at the document's address has the
//!   element's `id`, or its accessible name.

use std::time::Duration;

use agent_desktop_core::system::cdp_endpoint;
use agent_desktop_core::system::cdp_page::{
    NodeQuery, Outcome, PageAction, PageQuery, act_on_page,
};
use agent_desktop_core::{
    Action, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, StepMechanism,
};
use uiautomation::patterns::UIValuePattern;
use uiautomation::types::ControlType;

use crate::tree::automation::automation_client;
use crate::tree::element::UIAElement;

const CLICK_LABEL: &str = "DevTools.click";
const SET_VALUE_LABEL: &str = "DevTools.setValue";
const SELECT_LABEL: &str = "DevTools.select";
const CHECK_LABEL: &str = "DevTools.check";
const CHROMIUM_FRAMEWORK: &str = "Chrome";
const MAX_ANCESTORS: usize = 96;
const MAX_PORTS: usize = 8;
const PROBE_TIMEOUT: Duration = Duration::from_millis(250);
const ACTION_TIMEOUT: Duration = Duration::from_secs(4);

/// The steps of `action` done through DevTools, or `None` when this route
/// does not apply and nothing was done.
pub(crate) fn devtools_steps(
    element: &UIAElement,
    action: &Action,
    deadline: Deadline,
) -> Result<Option<Vec<ActionStep>>, AdapterError> {
    let (page_action, label) = match action {
        Action::Click => (PageAction::Click, CLICK_LABEL),
        Action::SetValue(value) => (PageAction::SetValue(value), SET_VALUE_LABEL),
        Action::Clear => (PageAction::SetValue(""), SET_VALUE_LABEL),
        Action::Select(value) => (PageAction::Select(value), SELECT_LABEL),
        Action::Check => (PageAction::Check(true), CHECK_LABEL),
        Action::Uncheck => (PageAction::Check(false), CHECK_LABEL),
        _ => return Ok(None),
    };
    let framework = element.0.get_framework_id().ok();
    if framework.as_deref() != Some(CHROMIUM_FRAMEWORK) {
        tracing::debug!(?framework, "devtools: not Chromium content");
        return Ok(None);
    }
    let Some(document) = only_document_above(element) else {
        tracing::debug!("devtools: not inside exactly one web document");
        return Ok(None);
    };
    let Some(port) = devtools_port(element) else {
        tracing::debug!("devtools: the browser has no DevTools endpoint");
        return Ok(None);
    };
    tracing::debug!(port, url = ?document.url, title = ?document.title, "devtools: page");
    let name = element.0.get_name().unwrap_or_default();
    let dom_id = element.0.get_automation_id().ok();
    let timeout = deadline.remaining().min(ACTION_TIMEOUT);
    let outcome = act_on_page(
        port,
        &PageQuery {
            url: document.url.as_deref(),
            title: document.title.as_deref(),
        },
        &NodeQuery {
            name: &name,
            dom_id: dom_id.as_deref(),
            editable: matches!(page_action, PageAction::SetValue(_) | PageAction::Select(_)),
        },
        &page_action,
        timeout,
    );
    match outcome {
        Ok(Outcome::NotFound) => {
            tracing::debug!("devtools: no single matching element on the page");
            Ok(None)
        }
        Ok(Outcome::NoSuchOption(options)) => Err(AdapterError::new(
            ErrorCode::ElementNotFound,
            "The list has no option with that text or value",
        )
        .with_details(serde_json::json!({ "kind": "no_such_option", "options": options }))
        .with_suggestion("Choose one of details.options")
        .with_disposition(DeliverySemantics::not_delivered())),
        Ok(Outcome::Done { verified: false }) if !matches!(page_action, PageAction::Click) => {
            Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "The page did not keep the change made through DevTools",
            )
            .with_disposition(DeliverySemantics::delivered_unverified()))
        }
        Ok(Outcome::Done { verified }) => Ok(Some(vec![
            ActionStep::succeeded(label)
                .with_mechanism(StepMechanism::SemanticApi)
                .with_verified(verified && !matches!(page_action, PageAction::Click)),
        ])),
        Err(unconfirmed) => Err(AdapterError::new(ErrorCode::ActionFailed, unconfirmed.0)
            .with_disposition(DeliverySemantics::delivered_unverified())),
    }
}

struct WebDocument {
    url: Option<String>,
    title: Option<String>,
}

/// The web document the element is in, when it is in exactly one: an element
/// of the browser's own interface has none, one inside a frame has several.
fn only_document_above(element: &UIAElement) -> Option<WebDocument> {
    let walker = automation_client().ok()?.get_control_view_walker().ok()?;
    let mut documents = Vec::new();
    let mut current = element.0.clone();
    for _ in 0..MAX_ANCESTORS {
        if current.get_control_type().ok() == Some(ControlType::Document) {
            documents.push(current.clone());
        }
        match walker.get_parent(&current) {
            Ok(parent) => current = parent,
            Err(_) => break,
        }
    }
    if documents.len() != 1 {
        return None;
    }
    let document = documents.remove(0);
    let url = document
        .get_pattern::<UIValuePattern>()
        .and_then(|pattern| pattern.get_value())
        .ok()
        .filter(|url| !url.is_empty());
    let title = document.get_name().ok().filter(|title| !title.is_empty());
    Some(WebDocument { url, title })
}

/// The loopback port of the element's process that answers as DevTools.
fn devtools_port(element: &UIAElement) -> Option<u16> {
    let pid = element.0.get_process_id().ok()?;
    crate::system::adapter::tcp_listener::loopback_listeners(pid)
        .into_iter()
        .take(MAX_PORTS)
        .find(|port| cdp_endpoint::endpoint_now(*port, PROBE_TIMEOUT).is_some())
}
