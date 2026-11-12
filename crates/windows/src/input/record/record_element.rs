//! Reads what a recorded action targeted: the element under the pointer or
//! holding focus, its window, its nearest identifiable ancestors, and how
//! many elements in that window share its identity. A `match_count` above 1
//! tells the consumer the recorded identity alone will be ambiguous on
//! replay and needs the ancestors to disambiguate.

use agent_desktop_core::{
    AdapterError, LocatorField, RecordRequest, Rect, WindowInfo, app_name_matches,
};
use serde_json::{Map, Value, json};
use uiautomation::patterns::{UISelectionItemPattern, UITogglePattern};
use uiautomation::types::{Handle, Point, ToggleState, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::{UIAutomation, UITreeWalker};
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::System::Console::GetConsoleWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, WindowFromPoint};

use super::record_window::{
    WindowBook, contains, display_bounds, root_of_handle, window_center, window_id_of,
};
use crate::tree::automation::automation_client;
use crate::tree::element::UIAElement;
use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::properties::{
    ElementProperties, PropertyOutcome, PropertyValue, read_live, read_one,
};
use crate::tree::roles::resolve_role;

const MAX_ANCESTORS: usize = 6;
const MAX_PARENT_STEPS: usize = 16;

/// An action's target. Either half can be missing: a point over a surface
/// with no UI Automation provider still has a window, and an element whose
/// window is not listed still has an identity.
pub(super) struct Target {
    pub(super) element: Option<UIAElement>,
    pub(super) window: Option<WindowInfo>,
    owner_id: Option<String>,
    root: isize,
    spot: Option<(i32, i32)>,
}

/// A text field's content when a typing run ends.
pub(super) enum FieldValue {
    Secret,
    Text(String),
    Unavailable,
}

pub(super) struct Observer {
    client: UIAutomation,
    walker: Option<UITreeWalker>,
    windows: WindowBook,
    own_pid: u32,
    own_console: Option<String>,
    app: Option<String>,
    window_id: Option<String>,
    display: Option<Rect>,
}

impl Observer {
    pub(super) fn new(request: &RecordRequest) -> Result<Self, AdapterError> {
        let client = automation_client()?;
        let walker = client.get_control_view_walker().ok();
        let console = root_of_handle(unsafe { GetConsoleWindow() } as isize);
        let display = match &request.display {
            Some(id) => Some(display_bounds(id)?),
            None => None,
        };
        Ok(Self {
            client,
            walker,
            windows: WindowBook::default(),
            own_pid: std::process::id(),
            own_console: (console != 0).then(|| window_id_of(console)),
            app: request.app.clone(),
            window_id: request.window_id.clone(),
            display,
        })
    }

    pub(super) fn at_point(&mut self, x: i32, y: i32) -> Target {
        let element = self
            .client
            .element_from_point(Point::new(x, y))
            .ok()
            .map(UIAElement::from);
        let handle = unsafe { WindowFromPoint(POINT { x, y }) } as isize;
        self.target(element, handle, Some((x, y)))
    }

    pub(super) fn focused(&mut self) -> Target {
        let element = self.client.get_focused_element().ok().map(UIAElement::from);
        let handle = element
            .as_ref()
            .and_then(|element| self.native_handle_of(element))
            .unwrap_or_else(|| unsafe { GetForegroundWindow() } as isize);
        let spot = window_center(root_of_handle(handle));
        self.target(element, handle, spot)
    }

    fn target(
        &mut self,
        element: Option<UIAElement>,
        handle: isize,
        spot: Option<(i32, i32)>,
    ) -> Target {
        let resolved = self.windows.resolve(handle, self.window_id.as_deref());
        Target {
            element,
            window: resolved.window,
            owner_id: resolved.owner_id,
            root: root_of_handle(handle),
            spot,
        }
    }

    /// Whether the target passes the session's `--app`, `--window-id` or
    /// `--display` filter. The recorder's own process and the console it runs
    /// in never pass, so stopping the session with Ctrl+C is not recorded.
    pub(super) fn keeps(&self, target: &Target) -> bool {
        let element_pid = target
            .element
            .as_ref()
            .and_then(|element| element.0.get_process_id().ok());
        if element_pid == Some(self.own_pid) {
            return false;
        }
        if let Some(display) = &self.display {
            let on_display = target.spot.is_some_and(|(x, y)| contains(display, x, y));
            if !on_display {
                return false;
            }
        }
        match &target.window {
            Some(window) if window.pid == self.own_pid => false,
            Some(window) if self.own_console.as_deref() == Some(window.id.as_str()) => false,
            Some(window) => {
                self.app
                    .as_deref()
                    .is_none_or(|app| app_name_matches(&window.app, app))
                    && self
                        .window_id
                        .as_deref()
                        .is_none_or(|id| window.id == id || target.owner_id.as_deref() == Some(id))
            }
            None => self.app.is_none() && self.window_id.is_none(),
        }
    }

    pub(super) fn same(&self, left: &UIAElement, right: &UIAElement) -> bool {
        self.client
            .compare_elements(&left.0, &right.0)
            .unwrap_or(false)
    }

    pub(super) fn describe(&self, target: &Target) -> Map<String, Value> {
        let mut fields = Map::new();
        if let Some(window) = &target.window {
            fields.insert("window".into(), window_json(window));
        }
        let Some(element) = &target.element else {
            return fields;
        };
        let (properties, _) = read_live(element);
        fields.insert("element".into(), element_json(&properties));
        let ancestors = self.ancestors(element);
        if !ancestors.is_empty() {
            fields.insert("ancestors".into(), Value::Array(ancestors));
        }
        if let Some(count) = self.match_count(&properties, target.root) {
            fields.insert("match_count".into(), json!(count));
        }
        if let Some(selection) = self.selection(element) {
            fields.insert("selection".into(), selection);
        }
        fields
    }

    pub(super) fn value(&self, element: &UIAElement) -> FieldValue {
        if read_one(element, TreeProperty::IsPassword).flag() == Some(true) {
            return FieldValue::Secret;
        }
        match read_one(element, TreeProperty::Value) {
            PropertyOutcome::Known(PropertyValue::Text(text)) => {
                let trimmed = text.strip_suffix('\r').unwrap_or(&text).to_string();
                FieldValue::Text(trimmed)
            }
            _ => FieldValue::Unavailable,
        }
    }

    fn ancestors(&self, element: &UIAElement) -> Vec<Value> {
        let Some(walker) = self.walker.as_ref() else {
            return Vec::new();
        };
        let mut found = Vec::new();
        let mut current = element.0.clone();
        for _ in 0..MAX_PARENT_STEPS {
            let Ok(parent) = walker.get_parent(&current) else {
                break;
            };
            let (properties, _) = read_live(&UIAElement::from(parent.clone()));
            let described = element_json(&properties);
            if described.get("role").and_then(Value::as_str) == Some("window") {
                break;
            }
            if described.get("name").is_some() || described.get("native_id").is_some() {
                found.push(described);
                if found.len() == MAX_ANCESTORS {
                    break;
                }
            }
            current = parent;
        }
        found
    }

    /// Counts the elements in the action's own top-level window (the popup,
    /// for an action inside one) that share the target's control type and
    /// identity.
    fn match_count(&self, properties: &ElementProperties, root: isize) -> Option<usize> {
        if root == 0 {
            return None;
        }
        let control_type = properties.get(TreeProperty::ControlType).number()?;
        let (property, value) = match (
            properties.get(TreeProperty::AutomationId).non_blank_text(),
            properties.get(TreeProperty::Name).non_blank_text(),
        ) {
            (Some(native_id), _) => (UIProperty::AutomationId, native_id),
            (None, Some(name)) => (UIProperty::Name, name),
            (None, None) => return None,
        };
        let by_type = self
            .client
            .create_property_condition(UIProperty::ControlType, Variant::from(control_type), None)
            .ok()?;
        let by_identity = self
            .client
            .create_property_condition(property, Variant::from(value), None)
            .ok()?;
        let condition = self
            .client
            .create_and_condition(by_type, by_identity)
            .ok()?;
        let root = self.client.element_from_handle(Handle::from(root)).ok()?;
        match root.find_all(TreeScope::Descendants, &condition) {
            Ok(found) => Some(found.len()),
            Err(_) => Some(0),
        }
    }

    fn selection(&self, element: &UIAElement) -> Option<Value> {
        let pattern = element.0.get_pattern::<UISelectionItemPattern>().ok()?;
        let mut selection = Map::new();
        if let Ok(container) = pattern.get_selection_container() {
            let (properties, _) = read_live(&UIAElement::from(container));
            selection.insert("container".into(), element_json(&properties));
        }
        Some(Value::Object(selection))
    }

    /// The state a click left: `checked` for a toggle (`mixed` when
    /// indeterminate) and `selected` for a selectable item.
    pub(super) fn state_after(&self, element: &UIAElement) -> Map<String, Value> {
        let mut state = Map::new();
        if let Ok(toggle) = element.0.get_pattern::<UITogglePattern>() {
            if let Ok(value) = toggle.get_toggle_state() {
                let checked = match value {
                    ToggleState::On => json!(true),
                    ToggleState::Off => json!(false),
                    ToggleState::Indeterminate => json!("mixed"),
                };
                state.insert("checked".into(), checked);
            }
        }
        if let Ok(item) = element.0.get_pattern::<UISelectionItemPattern>() {
            if let Ok(selected) = item.is_selected() {
                state.insert("selected".into(), json!(selected));
            }
        }
        state
    }

    /// The text a combo box shows as its current choice.
    pub(super) fn choice_value(&self, element: &UIAElement) -> Option<String> {
        read_one(element, TreeProperty::Value).non_blank_text()
    }

    fn native_handle_of(&self, element: &UIAElement) -> Option<isize> {
        let mut current = element.0.clone();
        for _ in 0..MAX_PARENT_STEPS {
            if let Ok(handle) = current.get_native_window_handle() {
                let raw: isize = handle.into();
                if raw != 0 {
                    return Some(raw);
                }
            }
            current = self.walker.as_ref()?.get_parent(&current).ok()?;
        }
        None
    }
}

fn window_json(window: &WindowInfo) -> Value {
    json!({
        "id": window.id,
        "title": window.title,
        "app_name": window.app,
        "pid": window.pid.get(),
    })
}

fn element_json(properties: &ElementProperties) -> Value {
    let mut element = Map::new();
    if let LocatorField::Known(role) = resolve_role(properties) {
        element.insert("role".into(), json!(role));
    }
    for (key, property) in [
        ("name", TreeProperty::Name),
        ("native_id", TreeProperty::AutomationId),
        ("class_name", TreeProperty::ClassName),
    ] {
        if let Some(text) = properties.get(property).non_blank_text() {
            element.insert(key.into(), json!(text));
        }
    }
    if let LocatorField::Known(bounds) = properties.get(TreeProperty::BoundingRectangle).bounds() {
        element.insert(
            "bounds".into(),
            json!({"x": bounds.x, "y": bounds.y, "width": bounds.width, "height": bounds.height}),
        );
    }
    Value::Object(element)
}
