//! Acting on a web page through its browser's DevTools protocol.
//!
//! An accessibility action on a Chromium page makes the browser bring its own
//! window to the front. The same click or value written through the DevTools
//! endpoint, when the browser was started with one, never activates anything:
//! the page is found by its address, the element by its accessible name (the
//! one the accessibility tree shows) or its `id`, and the action runs in the
//! page. When the page or exactly one matching element cannot be found, the
//! caller falls back to the accessibility action.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::cdp_socket::Socket;

/// The page an element belongs to, as its document reports it.
pub struct PageQuery<'a> {
    pub url: Option<&'a str>,
    pub title: Option<&'a str>,
}

/// The element: its accessible name, its `id` attribute when it has one, and
/// whether it must be something a value can be written into.
pub struct NodeQuery<'a> {
    pub name: &'a str,
    pub dom_id: Option<&'a str>,
    pub editable: bool,
}

pub enum PageAction<'a> {
    Click,
    SetValue(&'a str),
    /// Chooses the option of a list (`<select>`) whose text or value is this.
    Select(&'a str),
    /// Checks (`true`) or unchecks a checkbox, radio button or switch.
    Check(bool),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// No page, or not exactly one element, matched: nothing was done.
    NotFound,
    /// The action ran; `verified` is whether the page confirms its effect.
    Done { verified: bool },
    /// The list has no such option, so nothing was chosen. These are the
    /// texts of the options it has (at most 50).
    NoSuchOption(Vec<String>),
}

/// The action was sent but the page did not confirm it: it may have run.
#[derive(Debug)]
pub struct Unconfirmed(pub &'static str);

const LIST_TIMEOUT: Duration = Duration::from_millis(800);

const IS_ELEMENT: &str = "function(){return this.nodeType===1&&this.tagName!=='LABEL'}";
const IS_EDITABLE: &str = "function(){return this.nodeType===1&&(this.isContentEditable||\
    ((this.tagName==='INPUT'||this.tagName==='TEXTAREA'||this.tagName==='SELECT')&&!this.disabled&&!this.readOnly))}";
const CLICK: &str = "function(){this.click();return true}";
/// Chooses an option by its text, then by its value, then either ignoring case.
/// Anything other than a `<select>` answers `"other"`, and the caller falls back.
const SELECT: &str = "function(v){if(!(this instanceof HTMLSelectElement))return 'other';\
    var o=Array.from(this.options),l=v.trim().toLowerCase();\
    var m=o.find(function(x){return x.text.trim()===v.trim()})||o.find(function(x){return x.value===v})||\
    o.find(function(x){return x.text.trim().toLowerCase()===l||x.value.toLowerCase()===l});\
    if(!m)return {options:o.slice(0,50).map(function(x){return x.text.trim()})};\
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set.call(this,m.value);\
    this.dispatchEvent(new Event('input',{bubbles:true}));\
    this.dispatchEvent(new Event('change',{bubbles:true}));return this.value===m.value}";
/// Clicks a checkbox, radio button or switch only when it is not already as
/// wanted. Anything else answers `"other"`, and the caller falls back.
const CHECK: &str = "function(w){var r=this.getAttribute('role');\
    var box=this.type==='checkbox'||this.type==='radio';\
    if(!box&&r!=='checkbox'&&r!=='switch'&&r!=='radio')return 'other';\
    var c=function(e){return box?e.checked:e.getAttribute('aria-checked')==='true'};\
    if(c(this)!==w)this.click();return c(this)===w}";
/// Writes through the element's native setter and fires the events a typed
/// change fires, so a React or Vue form notices it.
const SET_VALUE: &str = "function(v){if(this.isContentEditable){this.textContent=v;\
    this.dispatchEvent(new InputEvent('input',{bubbles:true,inputType:'insertText',data:v}));\
    return this.textContent===v}\
    var p=this instanceof HTMLTextAreaElement?HTMLTextAreaElement.prototype:\
    this instanceof HTMLSelectElement?HTMLSelectElement.prototype:HTMLInputElement.prototype;\
    Object.getOwnPropertyDescriptor(p,'value').set.call(this,v);\
    this.dispatchEvent(new Event('input',{bubbles:true}));\
    this.dispatchEvent(new Event('change',{bubbles:true}));return this.value===v}";

/// Runs `action` on the one element of the one page that match, through the
/// DevTools endpoint on `port`.
pub fn act_on_page(
    port: u16,
    page: &PageQuery<'_>,
    node: &NodeQuery<'_>,
    action: &PageAction<'_>,
    timeout: Duration,
) -> Result<Outcome, Unconfirmed> {
    let until = Instant::now() + timeout;
    let Some(targets) =
        super::cdp_endpoint::get_json(port, "/json/list", LIST_TIMEOUT.min(timeout))
    else {
        return Ok(Outcome::NotFound);
    };
    let mut found: Option<(Session, String)> = None;
    for url in matching_pages(&targets, page) {
        let Some(mut session) = Session::connect(&url, until) else {
            continue;
        };
        match session.locate(node) {
            Located::One(object) if found.is_none() => found = Some((session, object)),
            Located::None => {}
            Located::One(_) | Located::Many => return Ok(Outcome::NotFound),
        }
    }
    match found {
        Some((mut session, object)) => session.perform(&object, action),
        None => Ok(Outcome::NotFound),
    }
}

/// Opens `pages` in a new browser window that is shown without being
/// activated, through the browser's own DevTools address. The first page
/// makes the window; the others open as tabs behind it.
pub fn open_in_background(websocket_url: &str, pages: &[String], timeout: Duration) -> bool {
    let Some(mut session) = Session::connect(websocket_url, Instant::now() + timeout) else {
        return false;
    };
    pages.iter().enumerate().all(|(index, url)| {
        session
            .call(
                "Target.createTarget",
                json!({ "url": url, "newWindow": index == 0, "background": true }),
            )
            .is_some_and(|result| result["targetId"].is_string())
    })
}

/// The DevTools addresses of the pages that are `page`: those at its address
/// (compared without the fragment), or failing that those with its title.
pub(crate) fn matching_pages(targets: &Value, page: &PageQuery<'_>) -> Vec<String> {
    let pages: Vec<&Value> = targets
        .as_array()
        .map(|all| {
            all.iter()
                .filter(|target| target["type"] == "page")
                .collect()
        })
        .unwrap_or_default();
    let socket = |target: &&Value| target["webSocketDebuggerUrl"].as_str().map(str::to_owned);
    let without_fragment = |url: &str| url.split('#').next().unwrap_or_default().to_owned();
    if let Some(url) = page.url.filter(|url| !url.is_empty()) {
        let wanted = without_fragment(url);
        let by_url: Vec<String> = pages
            .iter()
            .filter(|target| {
                target["url"]
                    .as_str()
                    .is_some_and(|url| without_fragment(url) == wanted)
            })
            .filter_map(socket)
            .collect();
        if !by_url.is_empty() {
            return by_url;
        }
    }
    match page.title.filter(|title| !title.is_empty()) {
        Some(title) => pages
            .iter()
            .filter(|target| target["title"].as_str() == Some(title))
            .filter_map(socket)
            .collect(),
        None => Vec::new(),
    }
}

enum Located {
    None,
    One(String),
    Many,
}

struct Session {
    socket: Socket,
    next: u64,
    until: Instant,
}

impl Session {
    fn connect(url: &str, until: Instant) -> Option<Self> {
        let timeout = until.checked_duration_since(Instant::now())?;
        Some(Self {
            socket: Socket::connect(url, timeout)?,
            next: 0,
            until,
        })
    }

    /// One protocol call; `None` when it fails, errors or times out.
    fn call(&mut self, method: &str, params: Value) -> Option<Value> {
        self.next += 1;
        let id = self.next;
        let message = json!({ "id": id, "method": method, "params": params }).to_string();
        if !self.socket.send_text(&message) {
            return None;
        }
        loop {
            let reply: Value = serde_json::from_str(&self.socket.read_text(self.until)?).ok()?;
            if reply["id"].as_u64() == Some(id) {
                return reply.get("result").cloned();
            }
        }
    }

    fn object(&mut self, expression: &str) -> Option<String> {
        let result = self.call(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": false }),
        )?;
        result["result"]["objectId"].as_str().map(str::to_owned)
    }

    fn passes(&mut self, object: &str, check: &str) -> bool {
        self.call(
            "Runtime.callFunctionOn",
            json!({ "objectId": object, "functionDeclaration": check, "returnByValue": true }),
        )
        .is_some_and(|result| result["result"]["value"] == true)
    }

    /// The element with that `id`, or the only element with that accessible
    /// name that the action can apply to.
    fn locate(&mut self, node: &NodeQuery<'_>) -> Located {
        let check = if node.editable {
            IS_EDITABLE
        } else {
            IS_ELEMENT
        };
        if let Some(id) = node.dom_id.filter(|id| !id.is_empty()) {
            let expression = format!("document.getElementById({})", Value::from(id));
            if let Some(object) = self.object(&expression)
                && self.passes(&object, check)
            {
                return Located::One(object);
            }
        }
        if node.name.is_empty() {
            return Located::None;
        }
        let Some(document) = self.object("document") else {
            return Located::None;
        };
        let _ = self.call("DOM.enable", json!({}));
        let Some(tree) = self.call(
            "Accessibility.queryAXTree",
            json!({ "objectId": document, "accessibleName": node.name }),
        ) else {
            return Located::None;
        };
        let backend_ids: Vec<i64> = tree["nodes"]
            .as_array()
            .map(|nodes| {
                nodes
                    .iter()
                    .filter(|node| node["ignored"] != true)
                    .filter_map(|node| node["backendDOMNodeId"].as_i64())
                    .collect()
            })
            .unwrap_or_default();
        let mut matches = Vec::new();
        for backend_id in backend_ids {
            let Some(resolved) =
                self.call("DOM.resolveNode", json!({ "backendNodeId": backend_id }))
            else {
                continue;
            };
            let Some(object) = resolved["object"]["objectId"].as_str() else {
                continue;
            };
            if self.passes(object, check) {
                matches.push(object.to_owned());
            }
        }
        match matches.len() {
            0 => Located::None,
            1 => Located::One(matches.remove(0)),
            _ => Located::Many,
        }
    }

    fn perform(&mut self, object: &str, action: &PageAction<'_>) -> Result<Outcome, Unconfirmed> {
        let (function, arguments) = match action {
            PageAction::Click => (CLICK, json!([])),
            PageAction::SetValue(value) => (SET_VALUE, json!([{ "value": value }])),
            PageAction::Select(value) => (SELECT, json!([{ "value": value }])),
            PageAction::Check(wanted) => (CHECK, json!([{ "value": wanted }])),
        };
        let result = self
            .call(
                "Runtime.callFunctionOn",
                json!({
                    "objectId": object,
                    "functionDeclaration": function,
                    "arguments": arguments,
                    "returnByValue": true,
                    "userGesture": true,
                }),
            )
            .ok_or(Unconfirmed("The page did not confirm the action"))?;
        if result.get("exceptionDetails").is_some() {
            return Err(Unconfirmed("The page raised an error while the action ran"));
        }
        Ok(outcome(&result["result"]["value"]))
    }
}

/// What the page answered: `true` or `false` for an action that ran and
/// whether its effect shows, `"other"` when the element is not the kind the
/// action is for, or the options of a list that has none like the one asked.
pub(crate) fn outcome(answer: &Value) -> Outcome {
    if answer == "other" {
        return Outcome::NotFound;
    }
    if let Some(options) = answer["options"].as_array() {
        return Outcome::NoSuchOption(
            options
                .iter()
                .filter_map(|o| o.as_str().map(str::to_owned))
                .collect(),
        );
    }
    Outcome::Done {
        verified: answer == true,
    }
}

#[cfg(test)]
#[path = "cdp_page_tests.rs"]
mod tests;
