//! Rows of the sidebar list: workspaces, their tabs and the group rows.
use super::*;
use super::menus::{creation_menu, workspace_group, workspace_menu};

/// How a workspace row is drawn this frame.
pub(super) struct RowState {
    rect: Rect,
    /// The row's position in the list and the list's length.
    position: (usize, usize),
    selected: bool,
    /// Elevation above the list: 0 at rest, 1 while dragged or settling.
    lift: f32,
    /// A row is held by the pointer, so rows give no hover feedback.
    dragging: bool,
    /// A terminal carried from the stage, which this row may receive.
    pane_drag: Option<PaneId>,
    /// The carried terminal's session can run in this workspace.
    accepts: bool,
}


pub(super) fn workspace_row(
    ui: &mut Ui,
    p: Palette,
    workspace: &WorkspaceView,
    view: &ChromeView,
    state: RowState,
    geometry: &mut Vec<(WorkspaceId, Rect)>,
    actions: &mut Vec<Action>,
) -> egui::Response {
    let RowState {
        rect: row,
        position,
        selected,
        lift,
        dragging,
        pane_drag,
        accepts,
    } = state;
    geometry.push((workspace.id, row));
    let response = ui.interact(
        row,
        ui.id().with(("workspace", workspace.id.get())),
        Sense::click_and_drag(),
    );
    let more_id = ui.id().with(("workspace-more", workspace.id.get()));
    let menu_open =
        egui::Popup::is_id_open(ui.ctx(), more_id.with("popup")) || response.context_menu_opened();
    let hovered = lift > 0.0 || (!dragging && (ui.rect_contains_pointer(row) || menu_open));
    let painter = if lift > 0.0 {
        // A lifted row floats: its shadow may fall outside the list.
        ui.painter()
            .with_clip_rect(ui.clip_rect().expand2(vec2(8.0, 12.0)))
    } else {
        ui.painter().clone()
    };
    if lift > 0.0 {
        let shadow = p.popup_shadow();
        painter.add(
            Shadow {
                color: shadow.color.gamma_multiply(lift),
                ..shadow
            }
            .as_shape(row, metrics::ROW_RADIUS),
        );
        painter.rect_filled(row, metrics::ROW_RADIUS, p.elevated.gamma_multiply(lift));
        painter.rect_stroke(
            row,
            metrics::ROW_RADIUS,
            Stroke::new(1.0, p.border.gamma_multiply(lift)),
            StrokeKind::Inside,
        );
    }
    if selected {
        painter.rect_filled(row, metrics::ROW_RADIUS, p.pressed);
    } else if hovered {
        painter.rect_filled(row, metrics::ROW_RADIUS, p.hover);
    }
    if response.has_focus() {
        painter.rect_stroke(
            row,
            metrics::ROW_RADIUS,
            Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    // A carried terminal can be dropped on any workspace but its own, as
    // long as that workspace is on the same machine.
    let receiving = pane_drag.filter(|_| !selected && accepts && ui.rect_contains_pointer(row));
    let receive = animate(
        ui.ctx(),
        ui.id().with(("workspace-drop", workspace.id.get())),
        receiving.is_some(),
        0.12,
    );
    if receive > 0.0 {
        painter.rect_filled(
            row,
            metrics::ROW_RADIUS,
            theme::tint(p.accent, 0.2 * receive),
        );
        painter.rect_stroke(
            row,
            metrics::ROW_RADIUS,
            Stroke::new(1.5, theme::tint(p.accent, 0.9 * receive)),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(pane) = receiving
        && ui.input(|input| input.pointer.primary_released())
    {
        actions.push(Action::MovePane(pane, Destination::Workspace(workspace.id)));
    }

    let text_left = row.left() + 9.0;
    // Every terminal here has stopped: a red dot leads the name.
    let name_left = if workspace.running {
        text_left
    } else {
        painter.circle_filled(Pos2::new(text_left + 3.0, row.center().y - 7.5), 3.0, p.red);
        text_left + 11.0
    };
    let hint = Binding::workspace(position.0)
        .map_or_else(String::new, |action| view.keybindings.hint(action));
    let show_shortcut = !hint.is_empty()
        && ui.input(|input| {
            input.focused
                && (input.modifiers.ctrl || input.modifiers.mac_cmd || input.modifiers.alt)
        });
    let hint_width = if show_shortcut {
        keycaps(
            &painter,
            Pos2::new(row.right() - 7.0, row.top() + 13.0),
            &hint,
            p.secondary,
        )
    } else {
        0.0
    };
    // An unread count takes the trailing edge; wider counts need more room.
    let trailing = if workspace.unread > 0 {
        crate::ui::notifications::pill_width(workspace.unread) + 14.0
    } else {
        30.0
    };
    let text_width = row.right() - text_left - trailing;
    galley_at(
        &painter,
        Pos2::new(name_left, row.center().y - 7.5),
        elided(
            &painter,
            &workspace.name,
            theme::medium(13.0),
            if selected || hovered || workspace.unread > 0 {
                p.fg
            } else {
                theme::mix(p.secondary, p.fg, 0.45)
            },
            row.right() - name_left - (hint_width + 14.0).max(trailing),
        ),
    );
    let detail = Pos2::new(text_left, row.center().y + 8.5);
    if let Some(alert) = &workspace.alert {
        // What the workspace is asking for, until it is read.
        icons::paint(
            &painter,
            Rect::from_center_size(detail + vec2(5.5, 0.0), Vec2::splat(11.0)),
            Icon::Bell,
            p.attention,
        );
        galley_at(
            &painter,
            detail + vec2(15.0, 0.0),
            elided(
                &painter,
                alert,
                theme::regular(11.0),
                p.secondary,
                text_width - 15.0,
            ),
        );
    } else if let Some(destination) = &workspace.remote {
        // A remote workspace shows its host where a local one shows its folder.
        icons::paint(
            &painter,
            Rect::from_center_size(detail + vec2(5.5, 0.0), Vec2::splat(11.0)),
            Icon::Globe,
            p.muted,
        );
        galley_at(
            &painter,
            detail + vec2(15.0, 0.0),
            elided(
                &painter,
                destination,
                theme::regular(11.0),
                p.muted,
                text_width - 15.0,
            ),
        );
    } else {
        // The branch trails the folder and may take most of the line: the
        // name above usually says which folder this is.
        let branch = workspace.branch.as_ref().map(|branch| {
            let dot = if branch.dirty { 9.0 } else { 0.0 };
            let name = elided(
                &painter,
                &branch.name,
                theme::regular(11.0),
                p.muted,
                (text_width * 0.6 - 14.0 - dot).max(0.0),
            );
            (name, branch.dirty, dot)
        });
        let branch_width = branch
            .as_ref()
            .map_or(0.0, |(name, _, dot)| 8.0 + 14.0 + name.size().x + dot);
        let path = painter.text(
            detail,
            Align2::LEFT_CENTER,
            helpers::path_label(
                &workspace.cwd,
                ((text_width - branch_width) / 5.9).max(4.0) as usize,
            ),
            theme::regular(11.0),
            p.muted,
        );
        if let Some((name, dirty, _)) = branch {
            let left = path.right() + 8.0;
            icons::paint(
                &painter,
                Rect::from_center_size(Pos2::new(left + 5.5, detail.y), Vec2::splat(11.0)),
                Icon::Branch,
                p.muted,
            );
            let named = galley_at(&painter, Pos2::new(left + 14.0, detail.y), name);
            // Work that is not committed yet marks the branch.
            if dirty {
                painter.circle_filled(Pos2::new(named.right() + 6.0, detail.y), 2.5, p.yellow);
            }
        }
    }

    response.widget_info(|| {
        let label = match workspace.unread {
            0 => workspace.name.clone(),
            1 => format!("{}, 1 unread notification", workspace.name),
            unread => format!("{}, {unread} unread notifications", workspace.name),
        };
        WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, label)
    });
    if response.clicked() {
        actions.push(Action::SelectWorkspace(workspace.id));
    }
    if response.double_clicked() {
        actions.push(Action::Rename(workspace.id));
    }
    response.context_menu(|ui| workspace_menu(ui, p, workspace, view, actions));

    let more = Rect::from_center_size(
        Pos2::new(
            row.right() - 17.0,
            if show_shortcut {
                row.bottom() - 12.0
            } else {
                row.center().y
            },
        ),
        Vec2::splat(24.0),
    );
    let more_response = ui.interact(more, more_id, Sense::click());
    more_response.widget_info(|| {
        WidgetInfo::labeled(
            WidgetType::Button,
            true,
            format!("Actions for {}", workspace.name),
        )
    });
    // While a row or a terminal is carried, rows are destinations, not controls.
    if (hovered && !dragging && pane_drag.is_none()) || more_response.has_focus() {
        if more_response.hovered() || menu_open {
            painter.rect_filled(more, 6, p.pressed);
        }
        icons::paint(
            &painter,
            more.shrink(5.0),
            Icon::Ellipsis,
            if more_response.hovered() {
                p.fg
            } else {
                p.secondary
            },
        );
    } else if workspace.unread > 0 {
        crate::ui::notifications::pill(
            &painter,
            Pos2::new(row.right() - 8.0, more.center().y),
            Align2::RIGHT_CENTER,
            workspace.unread,
            p,
        );
    } else if workspace.panes > 1 {
        painter.text(
            more.center(),
            Align2::CENTER_CENTER,
            workspace.panes.to_string(),
            theme::medium(11.0),
            p.muted,
        );
    }
    egui::Popup::menu(&more_response).show(|ui| workspace_menu(ui, p, workspace, view, actions));
    response
}

/// The workspace rows. Dragging one lifts it out of the list: it follows the
/// pointer while its neighbours ease aside, and takes the gap on release.
#[expect(
    clippy::too_many_arguments,
    reason = "row rendering also reports native drag anchors"
)]
pub(super) fn workspace_rows(
    ui: &mut Ui,
    p: Palette,
    view: &ChromeView,
    rows: &[&WorkspaceView],
    viewport: Rect,
    drag: &mut WorkspaceDrag,
    geometry: &mut Vec<(WorkspaceId, Rect)>,
    actions: &mut Vec<Action>,
) {
    let count = rows.len();
    let (list, _) = ui.allocate_exact_size(
        vec2(
            ui.available_width(),
            (count as f32 * ROW_STEP - (ROW_STEP - ROW_HEIGHT)).max(0.0),
        ),
        Sense::hover(),
    );
    let lifted = drag
        .lifted
        .and_then(|id| rows.iter().position(|w| w.id == id));
    if lifted.is_none() {
        // Nothing is lifted, or its workspace has closed.
        *drag = WorkspaceDrag::default();
    }
    // A carried terminal stays on the machine of the workspace it came from.
    let machine = view
        .workspaces
        .iter()
        .find(|workspace| Some(workspace.id) == view.active)
        .map(|workspace| &workspace.remote);
    let shown = |id: WorkspaceId, tops: &[(WorkspaceId, f32)]| {
        tops.iter().find(|(row, _)| *row == id).map(|(_, top)| *top)
    };

    // The held row follows the pointer, kept inside the list and its viewport.
    let mut held = lifted.zip(drag.grip).map(|(origin, grip)| {
        let pointer = drag
            .external
            .map(|drag| drag.pointer)
            .or_else(|| ui.input(|input| input.pointer.latest_pos()));
        let low = (viewport.top() - list.top()).max(0.0);
        let high = ((count - 1) as f32 * ROW_STEP)
            .min(viewport.bottom() - list.top() - ROW_HEIGHT)
            .max(low);
        let top = pointer
            .map(|pointer| pointer.y - grip.y - list.top())
            .or_else(|| shown(rows[origin].id, &drag.tops))
            .unwrap_or(origin as f32 * ROW_STEP)
            .clamp(low, high);
        if let Some(pointer) = pointer {
            let over = if list.top() < viewport.top() - 0.5 {
                (viewport.top() + SCROLL_REACH - pointer.y).max(0.0)
            } else {
                0.0
            } - if list.bottom() > viewport.bottom() + 0.5 {
                (pointer.y - viewport.bottom() + SCROLL_REACH).max(0.0)
            } else {
                0.0
            };
            if over != 0.0 {
                let speed = over.clamp(-2.0 * SCROLL_REACH, 2.0 * SCROLL_REACH) * 12.0;
                ui.scroll_with_delta_animation(
                    vec2(0.0, speed * ui.input(|input| input.stable_dt).min(0.1)),
                    ScrollAnimation::none(),
                );
                ui.ctx().request_repaint();
            }
        }
        (origin, top)
    });
    // Escape puts the row back; the shell does not receive that key.
    if held.is_some() && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
        drag.grip = None;
        held = None;
        ui.ctx().request_repaint();
    }
    let target = held.map(|(origin, top)| (origin, drop_index(top, count)));

    let decay = (-ui.input(|input| input.stable_dt).min(0.1) / ROW_SETTLE).exp();
    let mut tops = Vec::new();
    let mut moving = false;
    // The lifted row is drawn last, above its neighbours.
    for index in (0..count)
        .filter(|index| Some(*index) != lifted)
        .chain(lifted)
    {
        let workspace = rows[index];
        let global_index = view
            .workspaces
            .iter()
            .position(|w| w.id == workspace.id)
            .unwrap_or(index);
        let top = match held {
            Some((origin, top)) if origin == index => top,
            _ => {
                let rest = target.map_or(index, |(origin, target)| displaced(index, origin, target))
                    as f32
                    * ROW_STEP;
                // A row not yet tracked starts from its own place.
                let from = shown(workspace.id, &drag.tops).unwrap_or(index as f32 * ROW_STEP);
                let top = rest + (from - rest) * decay;
                if (top - rest).abs() < 0.5 {
                    rest
                } else {
                    moving = true;
                    top
                }
            }
        };
        if lifted.is_some() {
            tops.push((workspace.id, top));
        }
        let response = workspace_row(
            ui,
            p,
            workspace,
            view,
            RowState {
                rect: Rect::from_min_size(
                    Pos2::new(
                        list.left(),
                        if lifted.is_some() {
                            // Rows in motion stay on whole pixels, so their edges
                            // stay crisp at fractional display scales.
                            (list.top() + top).round_to_pixels(ui.pixels_per_point())
                        } else {
                            list.top() + top
                        },
                    ),
                    vec2(list.width(), ROW_HEIGHT),
                ),
                position: (global_index, view.workspaces.len()),
                selected: Some(workspace.id) == view.active,
                lift: animate(
                    ui.ctx(),
                    ui.id().with(("workspace-lift", workspace.id.get())),
                    Some(index) == lifted,
                    0.12,
                ),
                dragging: held.is_some(),
                pane_drag: view.pane_drag,
                accepts: Some(&workspace.remote) == machine,
            },
            geometry,
            actions,
        );
        if let Some((origin, target)) = target.filter(|(origin, _)| *origin == index) {
            if drag
                .external
                .map_or_else(|| response.drag_stopped(), |drag| drag.released)
            {
                if origin != target {
                    let global_target = view
                        .workspaces
                        .iter()
                        .position(|w| w.id == rows[target].id)
                        .unwrap_or(target);
                    actions.push(Action::MoveWorkspace(workspace.id, global_target));
                }
                drag.grip = None;
                drag.external = None;
                ui.ctx().request_repaint();
            } else if drag.external.is_some() || response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else {
                // The pointer was released while the row was not shown.
                drag.grip = None;
                ui.ctx().request_repaint();
            }
        } else if response.drag_started_by(PointerButton::Primary)
            && let Some(press) = ui.input(|input| input.pointer.press_origin())
        {
            drag.lifted = Some(workspace.id);
            drag.grip = Some(press - response.rect.min);
            ui.ctx().request_repaint();
        }
    }
    if moving {
        ui.ctx().request_repaint();
    }
    if held.is_none() && drag.grip.is_none() && !moving {
        // Every row is back in place.
        *drag = WorkspaceDrag::default();
    } else {
        drag.tops = tops;
    }
}


pub(super) fn group_row(
    ui: &mut Ui,
    p: Palette,
    view: &ChromeView,
    group: &WorkspaceGroup,
    openness: f32,
    dragging: bool,
    actions: &mut Vec<Action>,
) -> egui::Response {
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), GROUP_HEIGHT), Sense::hover());
    let plus = Rect::from_center_size(
        Pos2::new(row.right() - 17.0, row.center().y),
        Vec2::splat(28.0),
    );
    let response = ui.interact(
        row,
        ui.id().with(("workspace-group", group.id())),
        Sense::click_and_drag(),
    );
    let create = ui.interact(
        plus,
        ui.id().with(("group-create", group.id())),
        Sense::click(),
    );
    let hovered = !dragging && (ui.rect_contains_pointer(row) || response.context_menu_opened());
    let selected = view
        .workspaces
        .iter()
        .any(|w| w.group == Some(group.id()) && Some(w.id) == view.active);
    // Open folders show each workspace's own count; a closed one sums them.
    let unread: usize = view
        .workspaces
        .iter()
        .filter(|w| group.collapsed() && w.group == Some(group.id()))
        .map(|w| w.unread)
        .sum();
    let painter = ui.painter();
    if hovered {
        painter.rect_filled(row, metrics::ROW_RADIUS, p.hover);
    }
    if response.has_focus() {
        painter.rect_stroke(
            response.rect,
            metrics::ROW_RADIUS,
            Stroke::new(1.5, p.accent),
            StrokeKind::Inside,
        );
    }
    let center = Pos2::new(row.left() + 12.0, row.center().y);
    let rotation = egui::emath::Rot2::from_angle(openness * std::f32::consts::FRAC_PI_2);
    painter.add(egui::Shape::line(
        vec![
            center + rotation * vec2(-2.0, -4.0),
            center + rotation * vec2(2.0, 0.0),
            center + rotation * vec2(-2.0, 4.0),
        ],
        Stroke::new(1.5, p.muted),
    ));
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(row.left() + 30.0, row.center().y),
            Vec2::splat(16.0),
        ),
        Icon::Folder,
        if selected { p.accent } else { p.secondary },
    );
    galley_at(
        painter,
        Pos2::new(row.left() + 44.0, row.center().y),
        elided(
            painter,
            group.name(),
            theme::medium(13.0),
            if selected || hovered {
                p.fg
            } else {
                p.secondary
            },
            plus.left() - row.left() - 48.0,
        ),
    );
    let reveal = animate(
        ui.ctx(),
        create.id.with("reveal"),
        !dragging && (hovered || create.has_focus()),
        0.12,
    );
    if reveal > 0.0 {
        // The target spans the row's height; its highlight sits inside the row.
        let highlight = plus.shrink(3.0);
        if create.hovered() || create.is_pointer_button_down_on() {
            painter.rect_filled(highlight, 6, p.pressed);
        }
        if create.has_focus() {
            painter.rect_stroke(highlight, 6, Stroke::new(1.5, p.accent), StrokeKind::Inside);
        }
        icons::paint(
            painter,
            Rect::from_center_size(
                plus.center(),
                Vec2::splat(
                    16.0 * (0.25 + 0.75 * reveal)
                        * if create.is_pointer_button_down_on() {
                            0.96
                        } else {
                            1.0
                        },
                ),
            ),
            Icon::Plus,
            theme::tint(if create.hovered() { p.fg } else { p.secondary }, reveal),
        );
    } else {
        let count = view
            .workspaces
            .iter()
            .filter(|w| w.group == Some(group.id()))
            .count();
        if unread > 0 {
            crate::ui::notifications::pill(
                painter,
                Pos2::new(row.right() - 8.0, row.center().y),
                Align2::RIGHT_CENTER,
                unread,
                p,
            );
        } else if count > 0 {
            painter.text(
                plus.center(),
                Align2::CENTER_CENTER,
                count.to_string(),
                theme::medium(11.0),
                p.muted,
            );
        }
    }
    response.widget_info(|| {
        let label = match unread {
            0 => group.name().to_owned(),
            1 => format!("{}, 1 unread notification", group.name()),
            unread => format!("{}, {unread} unread notifications", group.name()),
        };
        WidgetInfo::selected(
            WidgetType::CollapsingHeader,
            true,
            !group.collapsed(),
            label,
        )
    });
    create.widget_info(|| {
        WidgetInfo::labeled(
            WidgetType::Button,
            true,
            format!("New workspace in {}", group.name()),
        )
    });
    if response.double_clicked() {
        actions.push(Action::RenameGroup(group.id()));
    } else if response.clicked() {
        actions.push(Action::SetGroupCollapsed(group.id(), !group.collapsed()));
    }
    if create
        .on_hover_text(format!("New workspace in {}", group.name()))
        .clicked()
    {
        actions.push(Action::NewInGroup(group.id()));
    }
    response
        .clone()
        .on_hover_cursor(CursorIcon::PointingHand)
        .context_menu(|ui| {
            creation_menu(ui, p, Some(group.id()), actions);
            menu_separator(ui, p);
            if menu_item(ui, p, Icon::Pencil, "Rename group…", "", false) {
                actions.push(Action::RenameGroup(group.id()));
                ui.close();
            }
            if menu_item(ui, p, Icon::Folder, "Default directory…", "", false) {
                actions.push(Action::GroupDefaultDirectory(group.id()));
                ui.close();
            }
            if menu_item(ui, p, Icon::Close, "Remove group", "", false) {
                actions.push(Action::RemoveGroup(group.id()));
                ui.close();
            }
        });
    response
}

#[expect(
    clippy::too_many_arguments,
    reason = "shared top-level drag presentation"
)]
pub(super) fn sidebar_item(
    ui: &mut Ui,
    p: Palette,
    view: &ChromeView,
    item: SidebarItem,
    viewport: Rect,
    workspace_drag: &mut WorkspaceDrag,
    lift: f32,
    dragging: bool,
    geometry: &mut Vec<(WorkspaceId, Rect)>,
    actions: &mut Vec<Action>,
) -> Option<egui::Response> {
    match item {
        SidebarItem::Group(id) => {
            let group = view.groups.iter().find(|g| g.id() == id)?;
            Some(workspace_group(
                ui,
                p,
                view,
                group,
                viewport,
                workspace_drag,
                dragging,
                geometry,
                actions,
            ))
        }
        SidebarItem::Workspace(id) => {
            let index = view
                .workspaces
                .iter()
                .position(|w| w.id == id && w.group.is_none())?;
            let workspace = &view.workspaces[index];
            let machine = view
                .workspaces
                .iter()
                .find(|w| Some(w.id) == view.active)
                .map(|w| &w.remote);
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
            Some(workspace_row(
                ui,
                p,
                workspace,
                view,
                RowState {
                    rect,
                    position: (index, view.workspaces.len()),
                    selected: Some(id) == view.active,
                    lift,
                    dragging,
                    pane_drag: view.pane_drag,
                    accepts: Some(&workspace.remote) == machine,
                },
                geometry,
                actions,
            ))
        }
    }
}

