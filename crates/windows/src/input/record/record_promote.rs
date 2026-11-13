//! Which control a press on a decoration belongs to.
//!
//! A toolbar button often draws its icon or caption as a child element, and
//! UI Automation's hit test returns that child: an unnamed `image` or
//! `statictext` that no replay can find. The press belongs to the control
//! that contains it, so the recorder reports the nearest actionable ancestor
//! instead, a few levels up at most.

/// How many parents the recorder climbs from a decoration.
pub(super) const PROMOTE_STEPS: usize = 3;

const DECORATIONS: &[&str] = &["image", "statictext", "text", "unknown"];

const ACTIONABLE: &[&str] = &[
    "button",
    "checkbox",
    "combobox",
    "link",
    "listitem",
    "menubutton",
    "menuitem",
    "radiobutton",
    "splitbutton",
    "switch",
    "tab",
    "treeitem",
];

/// Whether a hit element is only a decoration of its control: a leaf role
/// that carries no name of its own.
pub(super) fn is_decoration(role: Option<&str>, named: bool) -> bool {
    !named && role.is_none_or(|role| DECORATIONS.contains(&role))
}

/// Whether an ancestor is the control a press on its decoration belongs to.
pub(super) fn is_actionable(role: Option<&str>) -> bool {
    role.is_some_and(|role| ACTIONABLE.contains(&role))
}

#[cfg(test)]
#[path = "record_promote_tests.rs"]
mod tests;
