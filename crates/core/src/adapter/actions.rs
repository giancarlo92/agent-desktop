use crate::{
    AdapterError, InteractionLease, actions::action_request::ActionRequest,
    actions::action_result::ActionResult, adapter::native_handle::NativeHandle,
};

pub trait ActionOps: Send + Sync {
    fn headed_requirement_for_ref(
        &self,
        _entry: &crate::RefEntry,
        request: &ActionRequest,
        _deadline: crate::Deadline,
    ) -> Result<crate::HeadedRequirement, AdapterError> {
        Ok(request.headed_requirement())
    }

    fn headed_requirement(
        &self,
        _handle: &NativeHandle,
        request: &ActionRequest,
        _deadline: crate::Deadline,
    ) -> Result<crate::HeadedRequirement, AdapterError> {
        Ok(request.headed_requirement())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("execute_action"))
    }

    fn scroll_into_view(
        &self,
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        let _ = (handle, lease);
        Err(AdapterError::not_supported("scroll_into_view"))
    }

    /// Runs the command of an item of `window`'s menu bar, found by its path
    /// of item names, without opening the menu or taking the focus. Returns
    /// the item names as the menu shows them.
    fn invoke_menu(
        &self,
        _window: &crate::WindowInfo,
        _path: &[String],
        _lease: &InteractionLease,
    ) -> Result<Vec<String>, AdapterError> {
        Err(AdapterError::not_supported("invoke_menu"))
    }

    /// Clicks (or double-clicks) a point of `window`, in pixels of its
    /// screenshot, without moving the cursor or taking the focus.
    fn click_at(
        &self,
        _window: &crate::WindowInfo,
        _x: i32,
        _y: i32,
        _double: bool,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("click_at"))
    }

    /// Prepare a ref for dispatch. An adapter may return a strictly verified
    /// handle when its own scroll supplies independent evidence of the same
    /// native object's movement. Otherwise the caller resolves the original
    /// ref again; scrolling alone never authorizes rebinding a target.
    fn scroll_ref_into_view(
        &self,
        _entry: &crate::RefEntry,
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<Option<NativeHandle>, AdapterError> {
        self.scroll_into_view(handle, lease)?;
        Ok(None)
    }
}
