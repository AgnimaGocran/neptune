//! The sidebar list itself: the ordered items and where a drop lands.
use super::*;
use super::rows::sidebar_item;

#[expect(
    clippy::too_many_arguments,
    reason = "sidebar rendering also reports native drag anchors"
)]
pub(super) fn workspace_list(
    ui: &mut Ui,
    p: Palette,
    view: &ChromeView,
    viewport: Rect,
    workspace_drag: &mut WorkspaceDrag,
    drag: &mut SidebarDrag,
    geometry: &mut Vec<(WorkspaceId, Rect)>,
    actions: &mut Vec<Action>,
) {
    let items = view.sidebar_order;
    if workspace_drag.lifted.is_some_and(|id| {
        !view.workspaces.iter().any(|w| {
            w.id == id
                && w.group.is_some_and(|group| {
                    view.groups
                        .iter()
                        .any(|g| g.id() == group && !g.collapsed())
                })
        })
    }) {
        *workspace_drag = WorkspaceDrag::default();
    }
    let origin = drag
        .lifted
        .and_then(|id| items.iter().position(|item| *item == id));
    if origin.is_none() {
        drag.lifted = None;
        drag.grip = None;
        drag.external = None;
    }
    ui.spacing_mut().item_spacing.y = GROUP_GAP;
    let spacing = GROUP_GAP;
    let trailing_gap = items.last().map_or(0.0, |item| match item {
        SidebarItem::Workspace(_) => ROW_STEP - ROW_HEIGHT,
        SidebarItem::Group(_) => spacing,
    });
    let heights: Vec<_> = items
        .iter()
        .map(|item| {
            drag.positions
                .iter()
                .find(|(id, _, _)| id == item)
                .map_or_else(
                    || match item {
                        SidebarItem::Workspace(_) => ROW_STEP,
                        SidebarItem::Group(id) => view
                            .groups
                            .iter()
                            .find(|g| g.id() == *id)
                            .map_or(GROUP_HEIGHT + spacing, |group| {
                                let count = view
                                    .workspaces
                                    .iter()
                                    .filter(|w| w.group == Some(*id))
                                    .count();
                                GROUP_HEIGHT
                                    + spacing
                                    + if group.collapsed() {
                                        0.0
                                    } else {
                                        spacing + (count as f32 * ROW_STEP - 2.0).max(28.0)
                                    }
                            }),
                    },
                    |(_, _, height)| *height,
                )
        })
        .collect();
    if origin.is_none() {
        let list_top = ui.cursor().min;
        let mut next = 0.0;
        let mut positions = Vec::with_capacity(items.len());
        for (index, item) in items.iter().copied().enumerate() {
            let rect = Rect::from_min_size(
                list_top + vec2(0.0, next),
                vec2(ui.available_width(), heights[index]),
            );
            let mut child = ui.new_child(
                UiBuilder::new()
                    .id_salt(("sidebar-item", item))
                    .max_rect(rect),
            );
            let lift = animate(ui.ctx(), child.id().with("group-lift"), false, 0.12);
            if let Some(response) = sidebar_item(
                &mut child,
                p,
                view,
                item,
                viewport,
                workspace_drag,
                lift,
                false,
                geometry,
                actions,
            ) {
                let height = match item {
                    SidebarItem::Workspace(_) => ROW_STEP,
                    SidebarItem::Group(_) => child.min_rect().height() + spacing,
                };
                positions.push((item, next, height));
                next += height;
                if response.drag_started_by(PointerButton::Primary)
                    && let Some(press) = ui.input(|input| input.pointer.press_origin())
                {
                    drag.lifted = Some(item);
                    drag.grip = Some(press - response.rect.min);
                    *workspace_drag = WorkspaceDrag::default();
                    ui.ctx().request_repaint();
                }
            }
        }
        if !positions.is_empty() {
            ui.allocate_exact_size(
                vec2(ui.available_width(), (next - trailing_gap).max(0.0)),
                Sense::hover(),
            );
        }
        drag.positions = positions;
        return;
    }
    let list = Rect::from_min_size(
        ui.cursor().min,
        vec2(ui.available_width(), heights.iter().sum::<f32>()),
    );
    let mut held = origin.zip(drag.grip).map(|(origin, grip)| {
        let pointer = drag
            .external
            .map(|drag| drag.pointer)
            .or_else(|| ui.input(|input| input.pointer.latest_pos()));
        let low = (viewport.top() - list.top()).max(0.0);
        let high = (list.height() - heights[origin])
            .min(
                viewport.bottom()
                    - list.top()
                    - match items[origin] {
                        SidebarItem::Workspace(_) => ROW_HEIGHT,
                        SidebarItem::Group(_) => GROUP_HEIGHT,
                    },
            )
            .max(low);
        let top = pointer
            .map_or(0.0, |pos| pos.y - grip.y - list.top())
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
                ui.scroll_with_delta_animation(
                    vec2(
                        0.0,
                        over.clamp(-2.0 * SCROLL_REACH, 2.0 * SCROLL_REACH)
                            * 12.0
                            * ui.input(|input| input.stable_dt).min(0.1),
                    ),
                    ScrollAnimation::none(),
                );
                ui.ctx().request_repaint();
            }
        }
        (origin, top)
    });
    if held.is_some() && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
        drag.grip = None;
        held = None;
        ui.ctx().stop_dragging();
    }
    let target = held.map(|(origin, top)| sidebar_drop_index(top, &heights, origin));
    let mut order: Vec<_> = (0..items.len()).collect();
    if let Some((origin, target)) = origin.zip(target) {
        order.remove(origin);
        order.insert(target, origin);
    }
    let mut rests = vec![0.0; heights.len()];
    let mut next = 0.0;
    for index in order {
        rests[index] = next;
        next += heights[index];
    }
    let decay = (-ui.input(|input| input.stable_dt).min(0.1) / ROW_SETTLE).exp();
    let mut positions = Vec::new();
    let mut moving = false;
    for index in (0..items.len())
        .filter(|index| Some(*index) != origin)
        .chain(origin)
    {
        let item = items[index];
        let rest = rests[index];
        let top = if let Some((_, top)) = held.filter(|(origin, _)| *origin == index) {
            top
        } else if origin.is_some() {
            let from = drag
                .positions
                .iter()
                .find(|(id, _, _)| *id == item)
                .map_or(rest, |(_, top, _)| *top);
            let top = rest + (from - rest) * decay;
            if (top - rest).abs() < 0.5 {
                rest
            } else {
                moving = true;
                top
            }
        } else {
            rest
        };
        let rect = Rect::from_min_size(
            Pos2::new(
                list.left(),
                (list.top() + top).round_to_pixels(ui.pixels_per_point()),
            ),
            vec2(
                list.width(),
                match item {
                    SidebarItem::Workspace(_) => ROW_HEIGHT,
                    SidebarItem::Group(_) => heights[index] - spacing,
                },
            ),
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(("sidebar-item", item))
                .max_rect(rect),
        );
        let lift = animate(
            ui.ctx(),
            child.id().with("group-lift"),
            Some(index) == origin,
            0.12,
        );
        if lift > 0.0 && matches!(item, SidebarItem::Group(_)) {
            let painter = child
                .painter()
                .with_clip_rect(ui.clip_rect().expand2(vec2(8.0, 12.0)));
            let shadow = p.popup_shadow();
            painter.add(
                Shadow {
                    color: shadow.color.gamma_multiply(lift),
                    ..shadow
                }
                .as_shape(rect, metrics::ROW_RADIUS),
            );
            painter.rect_filled(rect, metrics::ROW_RADIUS, p.elevated.gamma_multiply(lift));
            painter.rect_stroke(
                rect,
                metrics::ROW_RADIUS,
                Stroke::new(1.0, p.border.gamma_multiply(lift)),
                StrokeKind::Inside,
            );
        }
        let mut idle = WorkspaceDrag::default();
        let response = sidebar_item(
            &mut child,
            p,
            view,
            item,
            viewport,
            if held.is_some() {
                &mut idle
            } else {
                &mut *workspace_drag
            },
            lift,
            held.is_some(),
            geometry,
            actions,
        );
        let height = match item {
            SidebarItem::Workspace(_) => ROW_STEP,
            SidebarItem::Group(_) => child.min_rect().height() + spacing,
        };
        positions.push((item, top, height));
        let Some(response) = response else {
            continue;
        };
        if let Some(target) = target.filter(|_| Some(index) == origin) {
            if drag
                .external
                .map_or_else(|| response.drag_stopped(), |drag| drag.released)
            {
                if index != target {
                    actions.push(Action::MoveSidebarItem(item, target));
                }
                drag.grip = None;
                drag.external = None;
                ui.ctx().request_repaint();
            } else if drag.external.is_some() || response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else {
                drag.grip = None;
                ui.ctx().request_repaint();
            }
        } else if response.drag_started_by(PointerButton::Primary)
            && let Some(press) = ui.input(|input| input.pointer.press_origin())
        {
            drag.lifted = Some(item);
            drag.grip = Some(press - response.rect.min);
            *workspace_drag = WorkspaceDrag::default();
            ui.ctx().request_repaint();
        }
    }
    let height = positions.iter().map(|(_, _, height)| height).sum::<f32>();
    if !positions.is_empty() {
        ui.allocate_exact_size(
            vec2(list.width(), (height - trailing_gap).max(0.0)),
            Sense::hover(),
        );
    }
    if moving {
        ui.ctx().request_repaint();
    }
    if held.is_none() && drag.grip.is_none() && !moving {
        drag.lifted = None;
        positions.clear();
    }
    drag.positions = positions;
}


