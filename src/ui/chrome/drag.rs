//! What a held or carried workspace keeps while it is dragged.
use super::*;

/// A workspace row lifted out of the sidebar list to be dropped elsewhere in it.
#[derive(Default)]
pub struct WorkspaceDrag {
    /// The row drawn above the others: held by the pointer, or settling into
    /// its place after release.
    pub(super) lifted: Option<WorkspaceId>,
    /// Original press relative to the lifted row's top-left corner.
    /// `None` once the row is released or the drag is cancelled.
    pub(super) grip: Option<Vec2>,
    /// Row tops relative to the list while a row is lifted; empty at rest.
    pub(super) tops: Vec<(WorkspaceId, f32)>,
    pub(super) external: Option<ExternalDrag>,
}


#[derive(Clone, Copy)]
pub(super) struct ExternalDrag {
    pub(super) pointer: Pos2,
    pub(super) released: bool,
}


impl WorkspaceDrag {
    pub(in crate::ui) fn workspace(&self) -> Option<WorkspaceId> {
        self.grip.and(self.lifted)
    }

    pub(in crate::ui) fn active(&self) -> bool {
        self.workspace().is_some()
    }
}


/// Top-level workspaces and folders share a drag list. A folder moves with its
/// visible children; measured geometry stays under stable entry identities.
#[derive(Default)]
pub struct SidebarDrag {
    pub(super) lifted: Option<SidebarItem>,
    pub(super) grip: Option<Vec2>,
    pub(super) positions: Vec<(SidebarItem, f32, f32)>,
    pub(super) external: Option<ExternalDrag>,
}


impl SidebarDrag {
    pub(in crate::ui) fn workspace(&self) -> Option<WorkspaceId> {
        match self.grip.and(self.lifted) {
            Some(SidebarItem::Workspace(id)) => Some(id),
            _ => None,
        }
    }

    pub(in crate::ui) fn active(&self) -> bool {
        self.grip.is_some() && self.lifted.is_some()
    }
}


impl UiState {
    pub(crate) fn carry_workspace(
        &mut self,
        workspace: WorkspaceId,
        grouped: bool,
        pointer: Pos2,
        grip: Vec2,
    ) {
        let external = Some(ExternalDrag {
            pointer,
            released: false,
        });
        if grouped {
            self.item_drag = Default::default();
            self.workspace_drag.lifted = Some(workspace);
            self.workspace_drag.grip = Some(grip);
            self.workspace_drag.external = external;
        } else {
            self.workspace_drag = Default::default();
            self.item_drag.lifted = Some(SidebarItem::Workspace(workspace));
            self.item_drag.grip = Some(grip);
            self.item_drag.external = external;
        }
    }

    pub(crate) fn finish_carried_workspace(&mut self) {
        if let Some(external) = &mut self.workspace_drag.external {
            external.released = true;
        }
        if let Some(external) = &mut self.item_drag.external {
            external.released = true;
        }
    }

    pub(crate) fn cancel_carried_workspace(&mut self) {
        if self.workspace_drag.external.is_some() {
            self.workspace_drag = Default::default();
        }
        if self.item_drag.external.is_some() {
            self.item_drag = Default::default();
        }
    }
}


/// Seconds a toggled sidebar takes to slide in or out.
pub(super) const SIDEBAR_SLIDE: f64 = 0.16;


/// A sidebar toggle in motion. It starts from however much of the sidebar was
/// showing, so a toggle reversed midway turns around without a jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarSlide {
    from: f32,
    started: f64,
}


impl SidebarSlide {
    /// Begins the slide that follows a toggle. `shown` is the state being left.
    pub fn toggled(previous: Option<Self>, shown: bool, now: f64) -> Self {
        Self {
            from: previous
                .and_then(|slide| slide.reveal(shown, now))
                .unwrap_or(if shown { 1.0 } else { 0.0 }),
            started: now,
        }
    }

    /// How much of the sidebar is shown at `now`, or `None` once it has settled.
    pub fn reveal(self, shown: bool, now: f64) -> Option<f32> {
        let progress = ((now - self.started) / SIDEBAR_SLIDE) as f32;
        (progress < 1.0).then(|| {
            egui::lerp(
                self.from..=if shown { 1.0 } else { 0.0 },
                egui::emath::easing::cubic_out(progress.max(0.0)),
            )
        })
    }
}


