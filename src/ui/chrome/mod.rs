//! Window chrome: the full-height sidebar, the toolbar and window controls.
use super::helpers::{
    self, animate, elided, galley_at, keycaps, menu_item, menu_layout, menu_separator, menu_submenu,
};
use super::{Action, UiState, WorkspaceView};
use crate::keybindings::{BindingAction as Binding, Keybindings};
use crate::{
    icons::{self, Icon},
    platform::window::{self, WindowOperation},
    theme::{self, Palette, metrics},
};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, Key, Layout, Modifiers, PointerButton, Pos2, Rect,
    Sense, Shadow, Stroke, StrokeKind, Ui, UiBuilder, Vec2, WidgetInfo, WidgetType,
    emath::GuiRounding as _, style::ScrollAnimation, vec2,
};
use neptune_model::{
    Destination, PaneId, SidebarItem, WindowId, WorkspaceGroup, WorkspaceGroupId, WorkspaceId,
};


/// What the chrome needs to know about the frame it surrounds.
pub struct ChromeView<'a> {
    pub keybindings: &'a Keybindings,
    pub workspaces: &'a [WorkspaceView],
    pub groups: &'a [WorkspaceGroup],
    pub sidebar_order: &'a [SidebarItem],
    /// Other open windows, with their current workspace names for destinations.
    pub windows: &'a [(WindowId, String)],
    pub active: Option<WorkspaceId>,
    pub pane: Option<PaneId>,
    pub pane_kind: neptune_model::PaneKind,
    /// The focused terminal's label and directory.
    pub subtitle: &'a str,
    /// The focused terminal's linked pull requests, while it has no tab to
    /// carry them.
    pub pull_requests: &'a [helpers::LinkedPullRequest],
    /// And the files its agent attached.
    pub attached: &'a [super::attached::File],
    /// The agents the focused terminal's agent started, likewise.
    pub spawned: &'a [super::agents::Spawned],
    /// The worktree of a terminal alone in view, which has no tab to name.
    pub worktree: Option<(neptune_model::PaneId, &'a super::worktrees::Tab)>,
    pub ports: &'a [crate::runtime::ports::Port],
    pub pane_generation: u64,
    pub zoomed: bool,
    /// The whole window. The window controls keep their place in it.
    pub window: Rect,
    /// How much of the sidebar is shown: 0 hidden, 1 shown, and between the
    /// two while a toggle slides it.
    pub sidebar: f32,
    /// The sidebar is shown once it has settled.
    pub sidebar_open: bool,
    /// The sidebar's full width.
    pub sidebar_width: f32,
    /// The window is wide enough to show a sidebar at all.
    pub sidebar_available: bool,
    /// A terminal of the active workspace is being carried by its header.
    pub pane_drag: Option<PaneId>,
    /// The trailing panel is shown, or `None` in a window too narrow for it.
    pub panel: Option<bool>,
    /// An agent waits for input where the panel does not show it.
    pub panel_attention: bool,
}


const SIDEBAR_WIDTH: std::ops::RangeInclusive<f32> = 170.0..=360.0;

const DEFAULT_SIDEBAR_WIDTH: f32 = 216.0;

const ROW_HEIGHT: f32 = 40.0;

/// The distance between the tops of neighbouring workspace rows.
const ROW_STEP: f32 = ROW_HEIGHT + 2.0;

const GROUP_HEIGHT: f32 = 28.0;

/// The space around a folder: above its workspaces and before the next item.
const GROUP_GAP: f32 = 4.0;

/// The workspace list starts this far below the toolbar strip.
const LIST_TOP: f32 = 4.0;

/// Time constant of a displaced row easing into place; it lands within 160 ms.
const ROW_SETTLE: f32 = 0.035;

/// Holding a dragged row this close to the list's edge scrolls the list.
const SCROLL_REACH: f32 = 28.0;


fn sidebar_drop_index(top: f32, heights: &[f32], origin: usize) -> usize {
    let mut slot = 0.0;
    let mut target = 0;
    for (_, height) in heights
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != origin)
    {
        if top < slot + height * 0.5 {
            break;
        }
        slot += height;
        target += 1;
    }
    target
}


/// The position a row would take if dropped with its top at `top`.
fn drop_index(top: f32, count: usize) -> usize {
    ((top / ROW_STEP).round().max(0.0) as usize).min(count.saturating_sub(1))
}


/// Where the row at `index` rests while the row from `origin` is held over
/// `target`: the rows between them close the gap it left and open one for it.
fn displaced(index: usize, origin: usize, target: usize) -> usize {
    if origin < index && index <= target {
        index - 1
    } else if target <= index && index < origin {
        index + 1
    } else {
        index
    }
}


/// Centre of the sidebar toggle from the window's leading edge: at the leading
/// margin while the sidebar is hidden, at its trailing edge while shown.
fn toggle_offset(view: &ChromeView) -> f32 {
    egui::lerp(22.0..=view.sidebar_width - 22.0, view.sidebar)
}


fn drag_region(ui: &mut Ui, rect: Rect, name: &str) {
    let drag = ui.interact(rect, ui.id().with(name), Sense::click_and_drag());
    if drag.drag_started() {
        window::send(ui.ctx(), WindowOperation::StartDrag);
    }
    if drag.double_clicked() {
        toggle_maximized(ui.ctx());
    }
}


fn toggle_maximized(ctx: &egui::Context) {
    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    window::send(ctx, WindowOperation::SetMaximized(!maximized));
}


fn icon_at(
    ui: &mut Ui,
    center: Pos2,
    icon: Icon,
    label: &str,
    hint: &str,
    salt: &str,
) -> egui::Response {
    ui.scope_builder(
        UiBuilder::new()
            .id_salt(salt)
            .max_rect(Rect::from_center_size(center, Vec2::splat(28.0))),
        |ui| icons::button_with_hint(ui, icon, label, hint),
    )
    .inner
}


pub(super) fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "·".into())
}



mod drag;
mod list;
mod menus;
mod rows;
mod sidebar;
mod toolbar;
#[cfg(test)]
mod tests;

pub use drag::{SidebarDrag, SidebarSlide, WorkspaceDrag};
pub use sidebar::sidebar;
pub use toolbar::{leading_controls, toolbar, window_controls};
