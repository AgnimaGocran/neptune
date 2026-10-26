//! The full-height sidebar and the handover of a held workspace.
use super::*;
use super::list::workspace_list;
use super::menus::creation_menu;

/// Hand a held workspace to the native host once it leaves its source list.
/// Reordering remains a release gesture; detaching begins while the button is
/// down so the new native window can take over the same drag.
pub(super) fn workspace_detach(
    ui: &mut Ui,
    sidebar: Rect,
    view: &ChromeView,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    if state.workspace_drag.external.is_some() || state.item_drag.external.is_some() {
        return;
    }
    if !state.dragging_sidebar_row() {
        return;
    }
    if state.overlay != crate::ui::OverlayState::None
        || !view.sidebar_open
        || view.sidebar < 1.0
        || !ui.input(|input| input.focused)
        || state
            .dragging_workspace()
            .is_some_and(|id| !view.workspaces.iter().any(|w| w.id == id))
        || ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
    {
        state.cancel_sidebar_drag(ui.ctx());
        return;
    }
    let Some(workspace) = state.dragging_workspace() else {
        return;
    };
    let pointer = ui.input(|input| {
        if !input.pointer.primary_down() && !input.pointer.primary_released() {
            return None;
        }
        let pointer = input.pointer.interact_pos()?;
        if !pointer.x.is_finite() || !pointer.y.is_finite() {
            return None;
        }
        let outside = !sidebar.contains(pointer);
        // Some native backends stop reporting motion as the pointer leaves.
        // PointerGone alone is not a detach: its last position must be at the
        // window edge, and the primary button must still be held.
        let gone = input
            .raw
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::PointerGone));
        let edge = pointer.x <= view.window.left() + 8.0
            || pointer.x >= view.window.right() - 8.0
            || pointer.y <= view.window.top() + 8.0
            || pointer.y >= view.window.bottom() - 8.0;
        (outside || (input.pointer.primary_down() && gone && edge)).then_some(pointer)
    });
    if let Some(pointer) = pointer {
        let grip = state.workspace_drag.grip.or(state.item_drag.grip);
        state.cancel_sidebar_drag(ui.ctx());
        actions.push(Action::DetachWorkspace {
            workspace,
            pointer: Some(pointer),
            grip,
        });
    }
}


/// The sidebar spans the full window height, like a native source list.
/// `state` holds its width while the edge is dragged, and a lifted workspace
/// row. While a toggle slides it, `rect` moves past the window's leading edge.
pub fn sidebar(
    ui: &mut Ui,
    rect: Rect,
    p: Palette,
    view: &ChromeView,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    state.workspace_rows.clear();
    // Read the captured row before the list can discard a response after a
    // native PointerGone, then again after it can start a new drag this frame.
    workspace_detach(ui, rect, view, state, actions);
    let strip = Rect::from_min_size(rect.min, vec2(rect.width(), metrics::TOOLBAR_HEIGHT));
    drag_region(ui, strip, "sidebar-drag");
    // Register the backdrop first so workspace and folder menus take precedence.
    let background = ui.interact(
        Rect::from_min_max(
            Pos2::new(rect.left() + 2.0, strip.bottom()),
            Pos2::new(rect.right() - 4.0, rect.bottom()),
        ),
        ui.id().with("sidebar-context"),
        Sense::click(),
    );
    background.context_menu(|ui| creation_menu(ui, p, None, actions));

    let footer_top = rect.bottom() - 46.0;
    let viewport = Rect::from_min_max(
        Pos2::new(rect.left() + 8.0, strip.bottom() + LIST_TOP),
        Pos2::new(rect.right() - 8.0, footer_top - 4.0),
    );
    ui.scope_builder(
        UiBuilder::new()
            .id_salt("workspace-list")
            .max_rect(viewport),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("workspaces")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    workspace_list(
                        ui,
                        p,
                        view,
                        viewport,
                        &mut state.workspace_drag,
                        &mut state.item_drag,
                        &mut state.workspace_rows,
                        actions,
                    );
                });
        },
    );
    workspace_detach(ui, rect, view, state, actions);

    // Footer: the primary creation action, with preferences beside it.
    let create = Rect::from_min_max(
        Pos2::new(rect.left() + 8.0, footer_top + 8.0),
        Pos2::new(rect.right() - 44.0, footer_top + 38.0),
    );
    let response = ui.interact(create, ui.id().with("workspace-create"), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "New workspace"));
    let painter = ui.painter();
    if response.is_pointer_button_down_on() {
        painter.rect_filled(create, metrics::ROW_RADIUS, p.pressed);
    } else if response.hovered() {
        painter.rect_filled(create, metrics::ROW_RADIUS, p.hover);
    }
    if response.has_focus() {
        painter.rect_stroke(
            create,
            metrics::ROW_RADIUS,
            Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    let ink = if response.hovered() {
        p.fg
    } else {
        p.secondary
    };
    icons::paint(
        painter,
        Rect::from_center_size(
            Pos2::new(create.left() + 15.0, create.center().y),
            Vec2::splat(14.0),
        ),
        Icon::Plus,
        ink,
    );
    galley_at(
        painter,
        Pos2::new(create.left() + 31.0, create.center().y),
        elided(
            painter,
            "New workspace",
            theme::medium(12.5),
            ink,
            create.width() - 36.0,
        ),
    );
    if response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(format!(
            "New workspace   {}",
            view.keybindings.hint(Binding::NewWorkspace)
        ))
        .clicked()
    {
        actions.push(Action::New);
    }
    if icon_at(
        ui,
        Pos2::new(rect.right() - 24.0, create.center().y),
        Icon::Settings,
        "Preferences",
        &view.keybindings.hint(Binding::Preferences),
        "sidebar-preferences",
    )
    .clicked()
    {
        actions.push(Action::Settings);
    }

    // The trailing edge resizes the sidebar once it rests there; the width is
    // saved on release.
    if view.sidebar < 1.0 {
        return;
    }
    let drag = &mut state.sidebar_drag;
    let handle = Rect::from_min_max(
        Pos2::new(rect.right() - 3.0, strip.bottom()),
        Pos2::new(rect.right() + 3.0, rect.bottom() - 12.0),
    );
    let resize = ui
        .interact(
            handle,
            ui.id().with("sidebar-resize"),
            Sense::click_and_drag(),
        )
        .on_hover_and_drag_cursor(CursorIcon::ResizeHorizontal);
    resize.widget_info(|| WidgetInfo::labeled(WidgetType::ResizeHandle, true, "Resize sidebar"));
    if resize.dragged()
        && let Some(pointer) = resize.interact_pointer_pos()
    {
        *drag = Some((pointer.x - rect.left()).clamp(*SIDEBAR_WIDTH.start(), *SIDEBAR_WIDTH.end()));
    }
    if resize.drag_stopped()
        && let Some(width) = drag.take()
    {
        actions.push(Action::SidebarWidth(width));
    }
    // A third quick click still means "reset", not a new gesture.
    if resize.double_clicked() || resize.triple_clicked() {
        *drag = None;
        actions.push(Action::SidebarWidth(DEFAULT_SIDEBAR_WIDTH));
    }
    let grip = animate(
        ui.ctx(),
        resize.id.with("grip"),
        resize.hovered() || resize.dragged(),
        0.12,
    );
    if grip > 0.0 {
        ui.painter().line_segment(
            [
                Pos2::new(rect.right(), strip.bottom() + 8.0),
                Pos2::new(rect.right(), rect.bottom() - 20.0),
            ],
            Stroke::new(
                2.0,
                theme::tint(
                    if resize.dragged() { p.accent } else { p.muted },
                    grip * 0.8,
                ),
            ),
        );
    }
}


