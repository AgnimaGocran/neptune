//! The menus of the chrome: a workspace, a group and the empty space.
use super::*;
use super::rows::{group_row, workspace_rows};

pub(super) fn workspace_menu(
    ui: &mut Ui,
    p: Palette,
    workspace: &WorkspaceView,
    view: &ChromeView,
    actions: &mut Vec<Action>,
) {
    menu_layout(ui, 210.0);
    let siblings: Vec<usize> = view
        .workspaces
        .iter()
        .enumerate()
        .filter(|(_, row)| row.group == workspace.group)
        .map(|(index, _)| index)
        .collect();
    let position = siblings
        .iter()
        .position(|index| view.workspaces[*index].id == workspace.id)
        .unwrap_or(0);
    if menu_item(ui, p, Icon::Pencil, "Rename…", "", false) {
        actions.push(Action::Rename(workspace.id));
        ui.close();
    }
    if menu_item(ui, p, Icon::Copy, "Duplicate", "", false) {
        actions.push(Action::CloneWorkspace(workspace.id));
        ui.close();
    }
    if menu_item(ui, p, Icon::ArrowUpRight, "Move to new window", "", false) {
        actions.push(Action::DetachWorkspace {
            workspace: workspace.id,
            pointer: None,
            grip: None,
        });
        ui.close();
    }
    if !view.windows.is_empty() {
        menu_submenu(ui, p, Icon::ArrowUpRight, "Move to window", |ui| {
            menu_layout(ui, 210.0);
            for (window, name) in view.windows {
                ui.push_id(window, |ui| {
                    if menu_item(ui, p, Icon::ArrowUpRight, name, "", false) {
                        actions.push(Action::MoveWorkspaceToWindow(workspace.id, *window));
                        ui.close();
                    }
                });
            }
        });
    }
    if position > 0 && menu_item(ui, p, Icon::ArrowUp, "Move up", "", false) {
        actions.push(Action::MoveWorkspace(workspace.id, siblings[position - 1]));
        ui.close();
    }
    if position + 1 < siblings.len() && menu_item(ui, p, Icon::ArrowDown, "Move down", "", false) {
        actions.push(Action::MoveWorkspace(workspace.id, siblings[position + 1]));
        ui.close();
    }
    if !view.groups.is_empty() {
        menu_submenu(ui, p, Icon::Folder, "Move to group", |ui| {
            menu_layout(ui, 210.0);
            if workspace.group.is_some() && menu_item(ui, p, Icon::Grid, "Ungrouped", "", false) {
                actions.push(Action::MoveToGroup(workspace.id, None));
                ui.close();
            }
            for group in view.groups {
                if Some(group.id()) != workspace.group {
                    ui.push_id(group.id(), |ui| {
                        if menu_item(ui, p, Icon::Folder, group.name(), "", false) {
                            actions.push(Action::MoveToGroup(workspace.id, Some(group.id())));
                            ui.close();
                        }
                    });
                }
            }
        });
    }
    if workspace.remote.is_some() {
        if menu_item(ui, p, Icon::Globe, "Disconnect from SSH", "", false) {
            actions.push(Action::Disconnect(workspace.id));
            ui.close();
        }
    } else if menu_item(ui, p, Icon::Globe, "Connect over SSH…", "", false) {
        actions.push(Action::Ssh(Some(workspace.id)));
        ui.close();
    }
    menu_separator(ui, p);
    if menu_item(ui, p, Icon::Close, "Close workspace", "", true) {
        actions.push(Action::CloseWorkspace(workspace.id));
        ui.close();
    }
}


pub(super) fn creation_menu(
    ui: &mut Ui,
    p: Palette,
    group: Option<WorkspaceGroupId>,
    actions: &mut Vec<Action>,
) {
    menu_layout(ui, 230.0);
    if menu_item(ui, p, Icon::Plus, "New workspace", "", false) {
        actions.push(group.map_or(Action::New, Action::NewInGroup));
        ui.close();
    }
    if menu_item(ui, p, Icon::Globe, "New SSH workspace", "", false) {
        actions.push(group.map_or(Action::Ssh(None), Action::SshInGroup));
        ui.close();
    }
    if group.is_none() && menu_item(ui, p, Icon::Folder, "New workspace group", "", false) {
        actions.push(Action::NewGroup);
        ui.close();
    }
}


#[expect(
    clippy::too_many_arguments,
    reason = "folder presentation and row drag state stay separate"
)]
pub(super) fn workspace_group(
    ui: &mut Ui,
    p: Palette,
    view: &ChromeView,
    group: &WorkspaceGroup,
    viewport: Rect,
    drag: &mut WorkspaceDrag,
    dragging: bool,
    geometry: &mut Vec<(WorkspaceId, Rect)>,
    actions: &mut Vec<Action>,
) -> egui::Response {
    let mut collapse = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.id().with("body"),
        !group.collapsed(),
    );
    collapse.set_open(!group.collapsed());
    let openness = collapse.openness(ui.ctx());
    let response = group_row(ui, p, view, group, openness, dragging, actions);
    if group.collapsed()
        && drag.lifted.is_some_and(|id| {
            view.workspaces
                .iter()
                .any(|w| w.id == id && w.group == Some(group.id()))
        })
    {
        *drag = WorkspaceDrag::default();
    }
    collapse.show_body_unindented(ui, |ui| {
        ui.set_opacity(openness);
        // A folder's children keep the row's full height and a quiet indent.
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_max(
                ui.cursor().min + vec2(14.0, 0.0),
                ui.max_rect().max,
            )),
            |ui| {
                let rows: Vec<_> = view
                    .workspaces
                    .iter()
                    .filter(|w| w.group == Some(group.id()))
                    .collect();
                let mut idle = WorkspaceDrag::default();
                let child_drag = if drag.lifted.is_none_or(|id| rows.iter().any(|w| w.id == id)) {
                    &mut *drag
                } else {
                    &mut idle
                };
                if rows.is_empty() {
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
                    ui.painter().text(
                        rect.left_center() + vec2(9.0, 0.0),
                        Align2::LEFT_CENTER,
                        "No workspaces",
                        theme::regular(11.5),
                        p.muted,
                    );
                } else {
                    workspace_rows(
                        ui,
                        p,
                        view,
                        &rows,
                        viewport.intersect(ui.clip_rect()),
                        child_drag,
                        geometry,
                        actions,
                    );
                }
            },
        );
    });
    response
}

