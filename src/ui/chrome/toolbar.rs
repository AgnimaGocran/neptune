//! The toolbar above the terminal, the window controls and the leading controls.
use super::*;

/// Minimize, maximize and close as three quiet lights at the window's trailing
/// edge, close outermost. Their glyphs appear when the pointer approaches, and
/// they fade to neutral in an inactive window.
pub fn window_controls(ui: &mut Ui, left_center: Pos2, p: Palette, actions: &mut Vec<Action>) {
    let group = Rect::from_min_max(
        left_center + vec2(-6.0, -12.0),
        left_center + vec2(58.0, 12.0),
    );
    let hovered = ui.rect_contains_pointer(group);
    let active = ui.input(|input| input.focused);
    let reveal = animate(ui.ctx(), ui.id().with("window-controls"), hovered, 0.12);
    for (index, (label, rgb)) in [
        ("Minimize", 0xfebc2e),
        ("Maximize", 0x28c840),
        ("Close window", 0xff5f57),
    ]
    .into_iter()
    .enumerate()
    {
        let center = left_center + vec2(6.0 + index as f32 * 20.0, 0.0);
        let response = ui.interact(
            Rect::from_center_size(center, Vec2::splat(20.0)),
            ui.id().with(("window-control", label)),
            Sense::click(),
        );
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
        let color = theme::color(rgb);
        let painter = ui.painter();
        let fill = if active || hovered {
            color
        } else {
            theme::mix(p.chrome, p.muted, 0.55)
        };
        painter.circle_filled(
            center,
            6.0,
            if response.is_pointer_button_down_on() {
                theme::mix(fill, Color32::BLACK, 0.2)
            } else {
                fill
            },
        );
        painter.circle_stroke(center, 6.0, Stroke::new(0.5, Color32::from_black_alpha(48)));
        if response.has_focus() {
            painter.circle_stroke(center, 8.5, Stroke::new(1.5, theme::tint(p.accent, 0.8)));
        }
        if reveal > 0.0 {
            let ink = Stroke::new(
                1.2,
                theme::tint(theme::mix(color, Color32::BLACK, 0.62), reveal),
            );
            match index {
                0 => {
                    painter.line_segment([center - vec2(3.0, 0.0), center + vec2(3.0, 0.0)], ink);
                }
                1 => {
                    painter.line_segment([center - vec2(3.0, 0.0), center + vec2(3.0, 0.0)], ink);
                    painter.line_segment([center - vec2(0.0, 3.0), center + vec2(0.0, 3.0)], ink);
                }
                _ => {
                    for flip in [1.0, -1.0] {
                        painter.line_segment(
                            [
                                center + vec2(-2.4, -2.4 * flip),
                                center + vec2(2.4, 2.4 * flip),
                            ],
                            ink,
                        );
                    }
                }
            }
        }
        if response.clicked() {
            match index {
                0 => window::send(ui.ctx(), WindowOperation::Minimize),
                1 => toggle_maximized(ui.ctx()),
                _ => actions.push(Action::WindowClose),
            }
        }
        response.on_hover_text(label);
    }
}


pub fn toolbar(
    ui: &mut Ui,
    rect: Rect,
    p: Palette,
    view: &ChromeView,
    state: &mut UiState,
    actions: &mut Vec<Action>,
) {
    drag_region(ui, rect, "title-drag");
    let middle = rect.center().y;
    // The title starts clear of the sidebar toggle: at the leading margin
    // while the sidebar is hidden, and at its trailing edge while it shows.
    let leading = if view.sidebar_available {
        toggle_offset(view) + 24.0
    } else {
        16.0
    };
    let left = (rect.left() + 8.0).max(view.window.left() + leading);

    // The command field stays centred on the content; narrow windows fall back
    // to an icon so the title keeps its room. A sliding sidebar decides this
    // from the width it is heading for, so the field does not swap midway.
    let field_width = (rect.width() * 0.32).clamp(210.0, 320.0);
    let settled_width = view.window.width()
        - if view.sidebar_open {
            view.sidebar_width
        } else {
            0.0
        };
    let show_field = settled_width >= 760.0;
    let field = Rect::from_center_size(rect.center(), vec2(field_width, 28.0));

    // The window controls hold the trailing 74 of the window; the cluster ends
    // clear of them.
    let mut right = rect.right() - 80.0;
    // "New workspace" moves here while the sidebar is away. It fades in from
    // the trailing edge so its neighbours make room gradually.
    let cluster = ui
        .scope_builder(
            UiBuilder::new()
                .id_salt("toolbar-actions")
                .max_rect(Rect::from_min_max(
                    Pos2::new(left, rect.top() + 8.0),
                    Pos2::new(
                        right
                            + if view.sidebar < 1.0 {
                                30.0 * view.sidebar
                            } else {
                                0.0
                            },
                        rect.bottom() - 8.0,
                    ),
                ))
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if view.sidebar < 1.0 {
                    ui.set_opacity(1.0 - view.sidebar);
                    let create = icons::button_with_hint(
                        ui,
                        Icon::Plus,
                        "New workspace",
                        &view.keybindings.hint(Binding::NewWorkspace),
                    );
                    ui.set_opacity(1.0);
                    if create.clicked() {
                        actions.push(Action::New);
                    }
                }
                if let Some(open) = view.panel
                    && crate::ui::panel::toggle(
                        ui,
                        p,
                        open,
                        view.panel_attention,
                        &view.keybindings.hint(Binding::ToggleRightPanel),
                    )
                {
                    actions.push(Action::Panel(crate::ui::panel::Event::Toggle));
                }
                if notification_bell(
                    ui,
                    p,
                    view.workspaces.iter().any(|w| w.unread > 0),
                    state.overlay == crate::ui::OverlayState::Notifications,
                ) {
                    actions.push(Action::Notifications);
                }
                if let Some(pane) = view.pane {
                    if icons::button_with_hint(
                        ui,
                        Icon::SplitHorizontal,
                        "Split below",
                        &view.keybindings.hint(Binding::SplitBelow),
                    )
                    .clicked()
                    {
                        actions.push(Action::Split(pane, neptune_model::Axis::Horizontal));
                    }
                    if icons::button_with_hint(
                        ui,
                        Icon::SplitVertical,
                        "Split right",
                        &view.keybindings.hint(Binding::SplitRight),
                    )
                    .clicked()
                    {
                        actions.push(Action::Split(pane, neptune_model::Axis::Vertical));
                    }
                    if icons::button_with_hint(
                        ui,
                        Icon::Terminal,
                        "New tab",
                        &view.keybindings.hint(Binding::NewTab),
                    )
                    .clicked()
                    {
                        actions.push(Action::NewTab(pane));
                    }
                    if icons::button_with_hint(
                        ui,
                        Icon::Globe,
                        "New browser tab",
                        &view.keybindings.hint(Binding::NewBrowser),
                    )
                    .clicked()
                    {
                        actions.push(Action::NewBrowser(pane, None, None));
                    }
                    if icons::button_with_hint(
                        ui,
                        Icon::Search,
                        "Find in terminal",
                        &view.keybindings.hint(Binding::Find),
                    )
                    .clicked()
                    {
                        actions.push(Action::Find);
                    }
                }
                if !show_field
                    && icons::button_with_hint(
                        ui,
                        Icon::Command,
                        "Command palette",
                        &view.keybindings.hint(Binding::CommandPalette),
                    )
                    .clicked()
                {
                    actions.push(Action::Palette);
                }
                ui.min_rect().left()
            },
        )
        .inner;
    right = cluster - 6.0;

    if view.zoomed {
        let chip = Rect::from_min_max(
            Pos2::new(right - 84.0, middle - 11.0),
            Pos2::new(right, middle + 11.0),
        );
        if chip.left() > left + 40.0 {
            let response = ui.interact(chip, ui.id().with("toolbar-zoom"), Sense::click());
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Exit zoom"));
            let painter = ui.painter();
            painter.rect_filled(
                chip,
                11,
                theme::tint(p.accent, if response.hovered() { 0.3 } else { 0.2 }),
            );
            icons::paint(
                painter,
                Rect::from_center_size(Pos2::new(chip.left() + 15.0, middle), Vec2::splat(12.0)),
                Icon::Minimize,
                p.accent,
            );
            painter.text(
                Pos2::new(chip.left() + 26.0, middle),
                Align2::LEFT_CENTER,
                "Zoomed",
                theme::medium(11.5),
                p.accent,
            );
            if response
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(format!(
                    "Show all terminals   {}",
                    view.keybindings.hint(Binding::ZoomPane)
                ))
                .clicked()
            {
                actions.push(Action::Zoom);
            }
            right = chip.left() - 8.0;
        }
    }

    if state.search_open && view.pane.is_some() {
        // Search takes the command field's place. In a narrow window it uses
        // the title's room instead, so its controls stay reachable.
        let field = if show_field {
            let width = (rect.width() * 0.42).clamp(300.0, 440.0);
            let centre = rect.center().x.min(right - 8.0 - width * 0.5);
            Rect::from_center_size(Pos2::new(centre, middle), vec2(width, 30.0))
        } else {
            Rect::from_min_max(
                Pos2::new(left + 6.0, middle - 15.0),
                Pos2::new(right - 4.0, middle + 15.0),
            )
        };
        if field.width() >= 150.0 {
            crate::ui::search::show(ui, field, p, view.pane_kind, state, actions);
        }
        if !show_field {
            return;
        }
        right = right.min(field.left() - 14.0);
    } else if show_field {
        let response = ui.interact(field, ui.id().with("toolbar-commands"), Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Command palette"));
        let painter = ui.painter();
        painter.rect_filled(
            field,
            metrics::CONTROL_RADIUS,
            if response.is_pointer_button_down_on() {
                p.pressed
            } else if response.hovered() {
                p.hover
            } else {
                p.control
            },
        );
        if response.has_focus() {
            painter.rect_stroke(
                field,
                metrics::CONTROL_RADIUS,
                Stroke::new(1.5, p.accent),
                egui::StrokeKind::Inside,
            );
        }
        icons::paint(
            painter,
            Rect::from_center_size(Pos2::new(field.left() + 16.0, middle), Vec2::splat(13.0)),
            Icon::Search,
            p.muted,
        );
        let caps = keycaps(
            painter,
            Pos2::new(field.right() - 6.0, middle),
            &view.keybindings.hint(Binding::CommandPalette),
            p.muted,
        );
        galley_at(
            painter,
            Pos2::new(field.left() + 30.0, middle),
            elided(
                painter,
                "Search commands",
                theme::regular(12.5),
                p.muted,
                field.width() - 44.0 - caps,
            ),
        );
        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            actions.push(Action::Palette);
        }
        right = right.min(field.left() - 14.0);
    }

    let Some(workspace) = view.workspaces.iter().find(|w| Some(w.id) == view.active) else {
        return;
    };
    let chips = helpers::PullRequestChips::layout(
        ui,
        ui.id().with("toolbar-pull-request"),
        view.pull_requests,
        (left + 96.0, right - 2.0, middle),
        p,
    );
    let spawned = helpers::SpawnedChip::layout(
        ui,
        ui.id().with("toolbar-spawned"),
        view.spawned,
        (left + 96.0, chips.left, middle),
        p,
    );
    let merged = crate::ui::worktrees::MergedChip::layout(
        ui,
        ui.id().with("toolbar-merged"),
        match view.worktree {
            Some((pane, tab)) => (pane, Some(tab)),
            None => (neptune_model::PaneId::new(0), None),
        },
        (left + 96.0, spawned.left, middle),
        p,
    );
    let attached = view.pane.map(|pane| {
        crate::ui::attached::AttachedChip::layout(
            ui,
            ui.id().with("toolbar-attached"),
            (pane, view.attached),
            (left + 96.0, merged.left, middle),
            p,
        )
    });
    let leading = attached.as_ref().map_or(merged.left, |chip| chip.left);
    let ports = crate::ui::ports::Chips::layout(
        ui,
        ui.id().with("toolbar-ports"),
        view.ports,
        view.pane.map(|pane| (pane, view.pane_generation)),
        (left + 42.0, leading, middle),
        p,
    );
    if ports.left < right - 2.0 {
        right = ports.left - 6.0;
    }
    chips.paint(ui.painter(), p, actions);
    spawned.paint(ui.painter(), p, actions);
    merged.paint(ui.painter(), p, actions);
    if let Some(attached) = attached {
        attached.paint(ui.painter(), p, actions);
    }
    ports.paint(ui.painter(), p, actions);
    let budget = right - left - 6.0;
    if budget < 36.0 {
        return;
    }
    let painter = ui.painter().with_clip_rect(Rect::from_min_max(
        Pos2::new(left, rect.top()),
        Pos2::new(right, rect.bottom()),
    ));
    let name = elided(&painter, &workspace.name, theme::medium(13.0), p.fg, budget);
    let name_width = name.size().x;
    galley_at(&painter, Pos2::new(left + 6.0, middle), name);
    let remaining = budget - name_width - 12.0;
    if remaining > 48.0 && !view.subtitle.is_empty() {
        galley_at(
            &painter,
            Pos2::new(left + 6.0 + name_width + 10.0, middle + 0.5),
            elided(
                &painter,
                view.subtitle,
                theme::regular(12.0),
                p.secondary,
                remaining,
            ),
        );
    }
}


/// The bell stays pressed while its popover is open, and carries a dot in the
/// attention colour while any terminal has an unread alert.
pub(super) fn notification_bell(ui: &mut Ui, p: Palette, unread: bool, open: bool) -> bool {
    let surface = ui.painter().add(egui::Shape::Noop);
    let response = icons::button(ui, Icon::Bell, "Notifications");
    if open {
        ui.painter().set(
            surface,
            egui::Shape::rect_filled(response.rect.shrink(1.0), 7, p.pressed),
        );
    }
    let dot = animate(ui.ctx(), response.id.with("unread"), unread, 0.16);
    if dot > 0.0 {
        // Cut out of the glyph, on whatever the button is showing beneath.
        let beneath = if open || response.is_pointer_button_down_on() {
            p.pressed
        } else if response.hovered() {
            p.hover
        } else {
            Color32::TRANSPARENT
        };
        let centre = response.rect.center() + vec2(5.0, -5.0);
        let painter = ui.painter();
        painter.circle_filled(centre, 5.0 * dot, p.chrome);
        painter.circle_filled(centre, 5.0 * dot, beneath);
        painter.circle_filled(centre, 3.5 * dot, p.attention);
    }
    response.clicked()
}


/// The window controls and the sidebar toggle, drawn over the sidebar and the
/// toolbar. The lights sit at the window's trailing edge; the toggle travels
/// with the sidebar's edge between its place there and the leading margin.
pub fn leading_controls(ui: &mut Ui, p: Palette, view: &ChromeView, actions: &mut Vec<Action>) {
    let origin = view.window.min;
    let middle = origin.y + metrics::TOOLBAR_HEIGHT * 0.5;
    // The lights' group spans 64 and ends 10 from the trailing edge, as it
    // began 10 from the leading edge before it moved there.
    window_controls(ui, Pos2::new(view.window.max.x - 68.0, middle), p, actions);
    if view.sidebar_available
        && icon_at(
            ui,
            Pos2::new(origin.x + toggle_offset(view).round(), middle),
            Icon::Sidebar,
            "Toggle sidebar",
            &view.keybindings.hint(Binding::ToggleSidebar),
            "sidebar-toggle",
        )
        .clicked()
    {
        actions.push(Action::ToggleSidebar);
    }
}


