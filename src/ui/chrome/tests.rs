//! The tests of the chrome, kept beside the code they cover.
    use super::*;
    use super::drag::SIDEBAR_SLIDE;
    use super::list::*;
    use super::menus::*;
    use super::rows::*;
    use super::toolbar::*;
    use crate::config::Config;
    use eframe::egui::Event;

    fn workspaces(ids: &[u64]) -> Vec<WorkspaceView> {
        ids.iter()
            .map(|id| WorkspaceView {
                unread: 0,
                alert: None,
                group: None,
                id: WorkspaceId::new(*id),
                name: format!("workspace {id}"),
                cwd: "/srv/app".into(),
                remote: None,
                branch: None,
                panes: 1,
                running: true,
            })
            .collect()
    }

    /// The vertical centre of the row at `index` in a list at rest.
    fn row(index: usize) -> Pos2 {
        Pos2::new(
            100.0,
            metrics::TOOLBAR_HEIGHT + LIST_TOP + index as f32 * ROW_STEP + ROW_HEIGHT * 0.5,
        )
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    /// A sidebar driven one frame at a time, applying moves as the app would.
    struct Fixture {
        ctx: egui::Context,
        order: Vec<u64>,
        groups: Vec<WorkspaceGroup>,
        membership: Vec<(u64, WorkspaceGroupId)>,
        sidebar_order: Vec<SidebarItem>,
        /// Window height; a short window makes the list scroll.
        height: f32,
        state: UiState,
        actions: Vec<Action>,
        /// Escape was still pending after the sidebar ran.
        escape_passed: bool,
    }

    impl Fixture {
        fn new(order: &[u64]) -> Self {
            Self::with_height(order, 600.0)
        }

        fn with_height(order: &[u64], height: f32) -> Self {
            let ctx = egui::Context::default();
            ctx.set_fonts(crate::platform::fonts::bundled_definitions());
            theme::apply(&ctx, &Config::default());
            let mut fixture = Self {
                ctx,
                order: order.into(),
                groups: Vec::new(),
                membership: Vec::new(),
                sidebar_order: order
                    .iter()
                    .map(|id| SidebarItem::Workspace(WorkspaceId::new(*id)))
                    .collect(),
                height,
                state: UiState::default(),
                actions: Vec::new(),
                escape_passed: false,
            };
            fixture.frame(vec![]);
            fixture
        }

        fn frame(&mut self, events: Vec<Event>) {
            let mut views = workspaces(&self.order);
            for workspace in &mut views {
                workspace.group = self
                    .membership
                    .iter()
                    .find(|(id, _)| *id == workspace.id.get())
                    .map(|(_, group)| *group);
            }
            let view = ChromeView {
                keybindings: &Default::default(),
                workspaces: &views,
                groups: &self.groups,
                sidebar_order: &self.sidebar_order,
                windows: &[],
                active: views.first().map(|workspace| workspace.id),
                pane: None,
                pane_kind: neptune_model::PaneKind::Terminal,
                subtitle: "",
                pull_requests: &[],
                attached: &[],
                spawned: &[],
                worktree: None,
                ports: &[],
                pane_generation: 1,
                zoomed: false,
                window: Rect::from_min_size(Pos2::ZERO, vec2(900.0, self.height)),
                sidebar: 1.0,
                sidebar_open: true,
                sidebar_width: 216.0,
                sidebar_available: true,
                pane_drag: None,
                panel: None,
                panel_attention: false,
            };
            let p = Palette::new(Config::default().theme);
            let side = Rect::from_min_size(Pos2::ZERO, vec2(216.0, self.height));
            let mut actions = Vec::new();
            let mut output = self.ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, self.height))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    sidebar(ui, side, p, &view, &mut self.state, &mut actions);
                    self.escape_passed = ui.input(|input| input.key_pressed(Key::Escape));
                },
            );
            output.textures_delta.clear();
            for action in &actions {
                let command = match action {
                    Action::SetGroupCollapsed(group, collapsed) => {
                        Some(neptune_model::Command::SetWorkspaceGroupCollapsed {
                            group: *group,
                            collapsed: *collapsed,
                        })
                    }
                    Action::MoveSidebarItem(item, index) => {
                        Some(neptune_model::Command::MoveSidebarItem {
                            item: *item,
                            index: *index,
                        })
                    }
                    Action::MoveWorkspace(workspace, index) => {
                        Some(neptune_model::Command::MoveWorkspace {
                            workspace: *workspace,
                            index: *index,
                        })
                    }
                    _ => None,
                };
                if let Some(command) = command {
                    let mut controller = neptune_model::Controller::new(self.model());
                    controller.dispatch(command).unwrap();
                    self.order = controller
                        .model()
                        .workspaces()
                        .iter()
                        .map(|w| w.id().get())
                        .collect();
                    self.groups = controller.model().groups().to_owned();
                    self.sidebar_order = controller.model().sidebar_order().to_owned();
                }
            }
            self.actions.extend(actions);
        }

        fn model(&self) -> neptune_model::Model {
            let specs = self
                .order
                .iter()
                .map(|id| neptune_model::WorkspaceSpec {
                    id: WorkspaceId::new(*id),
                    group: self
                        .membership
                        .iter()
                        .find(|(w, _)| w == id)
                        .map(|(_, g)| *g),
                    name: format!("workspace {id}"),
                    cwd: "/srv/app".into(),
                    remote: None,
                    panes: vec![neptune_model::PaneSpec {
                        kind: neptune_model::PaneKind::Terminal,
                        id: PaneId::new(*id),
                        cwd: "/srv/app".into(),
                        remote_cwd: None,
                        agent: None,
                        pull_requests: Vec::new(),
                        attachments: Vec::new(),
                        spawned_by: None,
                        project: None,
                        worktree: None,
                    }],
                    layout: neptune_model::Layout::pane(PaneId::new(*id)),
                    active: PaneId::new(*id),
                })
                .collect();
            neptune_model::Model::restore_ordered(
                specs,
                self.groups
                    .iter()
                    .map(|g| neptune_model::WorkspaceGroupSpec {
                        id: g.id(),
                        name: g.name().into(),
                        collapsed: g.collapsed(),
                        default_directory: g.default_directory().map(std::path::Path::to_path_buf),
                    })
                    .collect(),
                Some(self.sidebar_order.clone()),
                None,
                true,
                Default::default(),
            )
            .unwrap()
        }

        /// Presses a row and carries it to `to` over several frames.
        fn carry(&mut self, from: Pos2, to: Pos2) {
            self.frame(vec![Event::PointerMoved(from)]);
            self.frame(vec![button(from, true)]);
            for step in 1..=6 {
                self.frame(vec![Event::PointerMoved(from.lerp(to, step as f32 / 6.0))]);
            }
        }

        fn folders(&mut self, folders: &[(u64, bool)]) {
            self.groups = neptune_model::Model::restore_grouped(
                Vec::new(),
                folders
                    .iter()
                    .map(|(id, collapsed)| neptune_model::WorkspaceGroupSpec {
                        id: WorkspaceGroupId::new(*id),
                        name: format!("group {id}"),
                        collapsed: *collapsed,
                        default_directory: None,
                    })
                    .collect(),
                None,
                true,
                Default::default(),
            )
            .unwrap()
            .groups()
            .to_owned();
            self.sidebar_order = self
                .order
                .iter()
                .filter(|id| !self.membership.iter().any(|(w, _)| w == *id))
                .map(|id| SidebarItem::Workspace(WorkspaceId::new(*id)))
                .chain(self.groups.iter().map(|g| SidebarItem::Group(g.id())))
                .collect();
            self.order = self
                .model()
                .workspaces()
                .iter()
                .map(|w| w.id().get())
                .collect();
            for _ in 0..20 {
                self.frame(vec![]);
            }
        }

        fn group_moves(&self) -> Vec<(u64, usize)> {
            self.actions
                .iter()
                .filter_map(|a| match a {
                    Action::MoveSidebarItem(SidebarItem::Group(id), index) => {
                        Some((id.get(), *index))
                    }
                    _ => None,
                })
                .collect()
        }

        fn folder(&self, id: u64) -> Pos2 {
            let (_, top, _) = self
                .state
                .item_drag
                .positions
                .iter()
                .find(|(g, _, _)| *g == SidebarItem::Group(WorkspaceGroupId::new(id)))
                .unwrap();
            Pos2::new(
                100.0,
                metrics::TOOLBAR_HEIGHT + LIST_TOP + top + GROUP_HEIGHT * 0.5,
            )
        }

        fn top(&self, id: u64) -> Option<f32> {
            self.state
                .item_drag
                .positions
                .iter()
                .find(|(item, _, _)| *item == SidebarItem::Workspace(WorkspaceId::new(id)))
                .map(|(_, top, _)| *top)
        }

        fn moves(&self) -> Vec<(u64, usize)> {
            self.actions
                .iter()
                .filter_map(|action| match action {
                    Action::MoveWorkspace(id, index)
                    | Action::MoveSidebarItem(SidebarItem::Workspace(id), index) => {
                        Some((id.get(), *index))
                    }
                    _ => None,
                })
                .collect()
        }
    }

    #[test]
    fn folder_plus_creates_inside_its_group_without_toggling_or_selecting() {
        let mut sidebar = Fixture::new(&[1, 2]);
        let group = WorkspaceGroupId::new(7);
        sidebar.groups = neptune_model::Model::restore_grouped(
            Vec::new(),
            vec![neptune_model::WorkspaceGroupSpec {
                id: group,
                default_directory: None,
                name: "Projects".into(),
                collapsed: false,
            }],
            None,
            true,
            Default::default(),
        )
        .unwrap()
        .groups()
        .to_owned();
        sidebar.membership = vec![(2, group)];
        sidebar.sidebar_order = vec![
            SidebarItem::Workspace(WorkspaceId::new(1)),
            SidebarItem::Group(group),
        ];
        sidebar.frame(vec![]);
        let header_y = sidebar.folder(7).y;
        let plus = Pos2::new(216.0 - 8.0 - 17.0, header_y);
        sidebar.frame(vec![Event::PointerMoved(plus)]);
        sidebar.frame(vec![button(plus, true)]);
        sidebar.frame(vec![button(plus, false)]);
        assert!(matches!(sidebar.actions.as_slice(), [Action::NewInGroup(id)] if *id == group));
        sidebar.actions.clear();
        let header = Pos2::new(100.0, header_y);
        sidebar.frame(vec![Event::PointerMoved(header)]);
        sidebar.frame(vec![button(header, true)]);
        sidebar.frame(vec![button(header, false)]);
        assert!(
            matches!(sidebar.actions.as_slice(), [Action::SetGroupCollapsed(id, true)] if *id == group)
        );
        assert!(sidebar.groups[0].collapsed());
    }

    #[test]
    fn a_folder_can_be_dragged_above_ungrouped_workspaces() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        sidebar.membership = vec![(3, WorkspaceGroupId::new(7))];
        sidebar.folders(&[(7, false)]);
        let from = sidebar.folder(7);
        let to = row(0);
        sidebar.carry(from, to);
        assert!(sidebar.group_moves().is_empty());
        sidebar.frame(vec![button(to, false)]);
        assert_eq!(sidebar.group_moves(), [(7, 0)]);
        assert_eq!(sidebar.order, [3, 1, 2]);
        assert_eq!(
            sidebar.sidebar_order[0],
            SidebarItem::Group(WorkspaceGroupId::new(7))
        );
    }

    #[test]
    fn mixed_folders_and_workspaces_move_in_both_directions_and_cancel() {
        let mut sidebar = Fixture::new(&[1, 2, 3, 4]);
        sidebar.membership = vec![(3, WorkspaceGroupId::new(7)), (4, WorkspaceGroupId::new(7))];
        sidebar.folders(&[(7, false), (8, true)]);
        let to = row(1);
        sidebar.carry(sidebar.folder(7), to);
        sidebar.frame(vec![button(to, false)]);
        assert_eq!(
            sidebar.sidebar_order,
            [
                SidebarItem::Workspace(WorkspaceId::new(1)),
                SidebarItem::Group(WorkspaceGroupId::new(7)),
                SidebarItem::Workspace(WorkspaceId::new(2)),
                SidebarItem::Group(WorkspaceGroupId::new(8))
            ]
        );
        assert_eq!(sidebar.order, [1, 3, 4, 2]);
        for _ in 0..30 {
            sidebar.frame(vec![]);
        }
        let to = sidebar.folder(8) + vec2(0.0, GROUP_HEIGHT);
        sidebar.carry(row(0), to);
        sidebar.frame(vec![button(to, false)]);
        assert_eq!(
            sidebar.sidebar_order.last(),
            Some(&SidebarItem::Workspace(WorkspaceId::new(1)))
        );
        for _ in 0..30 {
            sidebar.frame(vec![]);
        }
        let from = sidebar.folder(8);
        sidebar.carry(from, row(0));
        sidebar.frame(vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        sidebar.frame(vec![button(row(0), false)]);
        assert!(!sidebar.escape_passed);
        assert_eq!(sidebar.group_moves(), [(7, 1)]);
        assert!(
            sidebar
                .groups
                .iter()
                .find(|g| g.id().get() == 8)
                .unwrap()
                .collapsed()
        );
    }

    #[test]
    fn grouped_workspace_drag_keeps_membership_and_mixed_sidebar_order() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        sidebar.membership = vec![(2, WorkspaceGroupId::new(7)), (3, WorkspaceGroupId::new(7))];
        sidebar.folders(&[(7, false)]);
        sidebar.sidebar_order.swap(0, 1);
        sidebar.order = sidebar
            .model()
            .workspaces()
            .iter()
            .map(|w| w.id().get())
            .collect();
        sidebar.frame(vec![]);
        let before = sidebar.sidebar_order.clone();
        let from = sidebar.folder(7) + vec2(0.0, GROUP_HEIGHT * 0.5 + GROUP_GAP + ROW_HEIGHT * 0.5);
        let to = from + vec2(0.0, ROW_STEP);
        sidebar.carry(from, to);
        sidebar.frame(vec![button(to, false)]);
        assert_eq!(sidebar.moves(), [(2, 1)]);
        assert_eq!(sidebar.order, [3, 2, 1]);
        assert_eq!(sidebar.sidebar_order, before);
        assert!(sidebar.group_moves().is_empty());
    }

    #[test]
    fn removing_the_held_folder_cancels_its_drag() {
        let mut sidebar = Fixture::new(&[1, 2]);
        sidebar.folders(&[(7, true)]);
        sidebar.carry(sidebar.folder(7), row(0));
        sidebar.groups.clear();
        sidebar
            .sidebar_order
            .retain(|item| !matches!(item, SidebarItem::Group(_)));
        sidebar.frame(vec![button(row(0), false)]);
        assert!(sidebar.group_moves().is_empty());
        assert!(sidebar.state.item_drag.lifted.is_none());
    }

    #[test]
    fn expanded_and_collapsed_folders_move_as_blocks_once_on_release() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        sidebar.membership = vec![
            (1, WorkspaceGroupId::new(7)),
            (2, WorkspaceGroupId::new(7)),
            (3, WorkspaceGroupId::new(8)),
        ];
        sidebar.folders(&[(7, false), (8, true), (9, false)]);
        let from = sidebar.folder(7);
        let to = sidebar.folder(9);
        sidebar.carry(from, to);
        assert!(sidebar.group_moves().is_empty());
        assert_eq!(
            sidebar.state.item_drag.lifted,
            Some(SidebarItem::Group(WorkspaceGroupId::new(7)))
        );
        sidebar.frame(vec![button(to, false)]);
        assert_eq!(sidebar.group_moves(), [(7, 2)]);
        assert_eq!(
            sidebar
                .groups
                .iter()
                .map(|g| g.id().get())
                .collect::<Vec<_>>(),
            [8, 9, 7]
        );
        assert_eq!(sidebar.order, [3, 1, 2]);
        assert!(sidebar.moves().is_empty());
        assert!(!sidebar.actions.iter().any(|a| matches!(
            a,
            Action::SetGroupCollapsed(..) | Action::SelectWorkspace(..) | Action::NewInGroup(..)
        )));
        for _ in 0..30 {
            sidebar.frame(vec![]);
        }
        assert!(sidebar.state.item_drag.lifted.is_none());
        let from = sidebar.folder(7);
        let to = sidebar.folder(8);
        sidebar.carry(from, to);
        sidebar.frame(vec![button(to, false)]);
        assert_eq!(sidebar.group_moves(), [(7, 2), (7, 0)]);
        assert!(sidebar.groups[1].collapsed());
    }

    #[test]
    fn cancelling_a_folder_drag_or_dropping_in_place_keeps_order() {
        let mut sidebar = Fixture::new(&[]);
        sidebar.folders(&[(7, true), (8, true), (9, true)]);
        let from = sidebar.folder(9);
        let to = sidebar.folder(7);
        sidebar.carry(from, to);
        sidebar.frame(vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        assert!(!sidebar.escape_passed);
        assert!(sidebar.state.item_drag.grip.is_none());
        sidebar.frame(vec![
            Event::PointerMoved(to + vec2(0.0, 15.0)),
            button(to, false),
        ]);
        for _ in 0..30 {
            sidebar.frame(vec![]);
        }
        assert!(sidebar.group_moves().is_empty());
        let from = sidebar.folder(8);
        let to = from + vec2(0.0, 8.0);
        sidebar.carry(from, to);
        sidebar.frame(vec![button(to, false)]);
        assert!(sidebar.group_moves().is_empty());
        assert_eq!(
            sidebar
                .groups
                .iter()
                .map(|g| g.id().get())
                .collect::<Vec<_>>(),
            [7, 8, 9]
        );
    }

    #[test]
    fn holding_a_folder_at_the_edge_scrolls_to_hidden_groups() {
        let mut sidebar = Fixture::with_height(&[], 300.0);
        sidebar.folders(&[
            (1, true),
            (2, true),
            (3, true),
            (4, true),
            (5, true),
            (6, true),
            (7, true),
            (8, true),
        ]);
        let from = sidebar.folder(1);
        let edge = Pos2::new(100.0, 300.0 - 54.0);
        sidebar.carry(from, edge);
        for _ in 0..120 {
            sidebar.frame(vec![]);
        }
        sidebar.frame(vec![button(edge, false)]);
        assert_eq!(sidebar.group_moves(), [(1, 7)]);
    }

    #[test]
    fn rows_make_room_for_a_dragged_workspace() {
        assert_eq!(drop_index(-40.0, 4), 0);
        assert_eq!(drop_index(ROW_STEP * 0.49, 4), 0);
        assert_eq!(drop_index(ROW_STEP * 0.51, 4), 1);
        assert_eq!(drop_index(ROW_STEP * 9.0, 4), 3);
        assert_eq!(drop_index(0.0, 0), 0);
        let rests = |origin, target| -> Vec<usize> {
            (0..5).map(|i| displaced(i, origin, target)).collect()
        };
        // The held row's own entry is unused; its neighbours close the gap.
        assert_eq!(rests(1, 3), [0, 1, 1, 2, 4]);
        assert_eq!(rests(3, 0), [1, 2, 3, 3, 4]);
        assert_eq!(rests(2, 2), [0, 1, 2, 3, 4]);
    }

    #[test]
    fn dragging_a_row_moves_its_workspace_once_and_the_list_settles() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        let to = row(2) + vec2(0.0, 6.0);
        sidebar.carry(row(0), to);
        assert_eq!(
            sidebar.state.item_drag.lifted,
            Some(SidebarItem::Workspace(WorkspaceId::new(1)))
        );
        assert!(sidebar.moves().is_empty(), "nothing moves before release");
        // The held row tracks the pointer; its neighbours ease up to make room.
        assert_eq!(sidebar.top(1), Some(2.0 * ROW_STEP));
        for _ in 0..20 {
            sidebar.frame(vec![]);
        }
        assert_eq!(sidebar.top(2), Some(0.0));
        assert_eq!(sidebar.top(3), Some(ROW_STEP));

        sidebar.frame(vec![button(to, false)]);
        assert_eq!(sidebar.moves(), [(1, 2)]);
        assert_eq!(sidebar.order, [2, 3, 1]);
        for _ in 0..20 {
            sidebar.frame(vec![]);
        }
        let rest = &sidebar.state.item_drag;
        assert!(rest.lifted.is_none() && rest.grip.is_none());
        assert_eq!(sidebar.moves().len(), 1);
        assert!(
            !sidebar
                .actions
                .iter()
                .any(|action| matches!(action, Action::SelectWorkspace(_))),
            "a drag is not a click"
        );
    }

    #[test]
    fn a_click_still_selects_and_a_row_dropped_in_place_does_not_move() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        sidebar.frame(vec![Event::PointerMoved(row(1))]);
        sidebar.frame(vec![button(row(1), true)]);
        sidebar.frame(vec![button(row(1), false)]);
        assert!(matches!(
            sidebar.actions[..],
            [Action::SelectWorkspace(id)] if id == WorkspaceId::new(2)
        ));

        // Lifted, but released before it passes the middle of a neighbour.
        let to = row(1) + vec2(0.0, 20.0);
        sidebar.carry(row(1), to);
        assert_eq!(
            sidebar.state.item_drag.lifted,
            Some(SidebarItem::Workspace(WorkspaceId::new(2)))
        );
        sidebar.frame(vec![button(to, false)]);
        assert!(sidebar.moves().is_empty());
        assert_eq!(sidebar.order, [1, 2, 3]);
    }

    #[test]
    fn holding_a_row_at_the_edge_scrolls_to_positions_out_of_view() {
        // Just over four of eight rows fit: the last position starts hidden.
        let mut sidebar = Fixture::with_height(&[1, 2, 3, 4, 5, 6, 7, 8], 300.0);
        let edge = Pos2::new(100.0, 300.0 - 54.0);
        sidebar.carry(row(0), edge);
        let reachable = sidebar.top(1).unwrap();
        assert!(reachable < 4.0 * ROW_STEP, "the row stays inside the view");
        for _ in 0..120 {
            sidebar.frame(vec![]);
        }
        assert_eq!(sidebar.top(1), Some(7.0 * ROW_STEP));
        sidebar.frame(vec![button(edge, false)]);
        assert_eq!(sidebar.moves(), [(1, 7)]);

        // And back up: the list scrolls the other way under a row held at the top.
        for _ in 0..20 {
            sidebar.frame(vec![]);
        }
        let top = Pos2::new(100.0, metrics::TOOLBAR_HEIGHT + LIST_TOP);
        sidebar.carry(edge - vec2(0.0, 20.0), top);
        for _ in 0..120 {
            sidebar.frame(vec![]);
        }
        sidebar.frame(vec![button(top, false)]);
        assert_eq!(sidebar.moves(), [(1, 7), (1, 0)]);
        assert_eq!(sidebar.order, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn escape_puts_a_dragged_row_back_and_is_not_passed_on() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        let to = row(0) + vec2(0.0, 6.0);
        sidebar.carry(row(2), to);
        assert!(sidebar.state.item_drag.grip.is_some());
        sidebar.frame(vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        assert!(!sidebar.escape_passed, "the shell must not see this Escape");
        assert!(sidebar.state.item_drag.grip.is_none());
        sidebar.frame(vec![button(to, false)]);
        assert!(sidebar.moves().is_empty());
        assert_eq!(sidebar.order, [1, 2, 3]);
    }

    fn detached(actions: &[Action]) -> Vec<(WorkspaceId, Option<Pos2>)> {
        actions
            .iter()
            .filter_map(|action| match action {
                Action::DetachWorkspace {
                    workspace, pointer, ..
                } => Some((*workspace, *pointer)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_workspace_leaving_the_sidebar_detaches_while_held_once_without_reordering() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        let to = Pos2::new(280.0, row(2).y);
        sidebar.carry(row(0), to);
        let detach = detached(&sidebar.actions);
        assert_eq!(detach.len(), 1);
        assert_eq!(detach[0].0, WorkspaceId::new(1));
        assert!(detach[0].1.is_some_and(|pointer| pointer.x > 216.0));
        assert!(sidebar.ctx.input(|input| input.pointer.primary_down()));
        assert!(!sidebar.state.dragging_sidebar_row());
        sidebar.frame(vec![button(to, false)]);
        sidebar.frame(vec![]);
        assert_eq!(detached(&sidebar.actions), detach);
        assert!(sidebar.moves().is_empty());
        assert_eq!(sidebar.order, [1, 2, 3]);
    }

    #[test]
    fn motion_and_release_beyond_the_sidebar_in_one_frame_still_detach_without_reordering() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        sidebar.carry(row(0), Pos2::new(210.0, row(2).y));
        let outside = Pos2::new(280.0, row(2).y);
        sidebar.frame(vec![Event::PointerMoved(outside), button(outside, false)]);
        assert_eq!(
            detached(&sidebar.actions),
            [(WorkspaceId::new(1), Some(outside))]
        );
        assert!(sidebar.moves().is_empty());
    }

    #[test]
    fn a_grouped_workspace_detaches_but_its_folder_never_does() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        sidebar.membership = vec![(2, WorkspaceGroupId::new(7)), (3, WorkspaceGroupId::new(7))];
        sidebar.folders(&[(7, false)]);
        let from = sidebar.folder(7) + vec2(0.0, GROUP_HEIGHT * 0.5 + GROUP_GAP + ROW_HEIGHT * 0.5);
        let to = Pos2::new(280.0, from.y);
        sidebar.carry(from, to);
        assert_eq!(
            detached(&sidebar.actions)
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            [WorkspaceId::new(2)]
        );
        sidebar.frame(vec![button(to, false)]);
        assert!(sidebar.moves().is_empty());
        assert!(sidebar.group_moves().is_empty());

        let mut sidebar = Fixture::new(&[1, 2]);
        sidebar.folders(&[(7, true)]);
        let from = sidebar.folder(7);
        let to = Pos2::new(280.0, from.y);
        sidebar.carry(from, to);
        sidebar.frame(vec![button(to, false)]);
        assert!(detached(&sidebar.actions).is_empty());
    }

    #[test]
    fn approaching_the_sidebar_edge_preserves_normal_workspace_reordering() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        let near = Pos2::new(210.0, row(2).y);
        sidebar.carry(row(0), near);
        assert!(detached(&sidebar.actions).is_empty());
        assert!(sidebar.state.dragging_sidebar_row());
        sidebar.frame(vec![Event::PointerMoved(row(2))]);
        sidebar.frame(vec![button(row(2), false)]);
        assert_eq!(sidebar.moves(), [(1, 2)]);
        assert!(detached(&sidebar.actions).is_empty());
    }

    #[test]
    fn escape_or_an_opening_sheet_cancels_before_a_workspace_can_detach() {
        for sheet in [false, true] {
            let mut sidebar = Fixture::new(&[1, 2]);
            let near = Pos2::new(210.0, row(1).y);
            sidebar.carry(row(0), near);
            if sheet {
                sidebar.state.overlay = super::super::OverlayState::Settings;
                sidebar.frame(vec![]);
            } else {
                sidebar.frame(vec![Event::Key {
                    key: Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }]);
                assert!(!sidebar.escape_passed);
            }
            assert!(!sidebar.state.dragging_sidebar_row());
            let outside = Pos2::new(280.0, near.y);
            sidebar.frame(vec![Event::PointerMoved(outside)]);
            sidebar.frame(vec![button(outside, false)]);
            assert!(detached(&sidebar.actions).is_empty());
            assert!(sidebar.moves().is_empty());
        }
    }

    #[test]
    fn a_workspace_removed_during_its_drag_cannot_detach_a_replacement_row() {
        let mut sidebar = Fixture::new(&[1, 2, 3]);
        sidebar.carry(row(0), Pos2::new(210.0, row(1).y));
        sidebar.order.remove(0);
        sidebar.sidebar_order.remove(0);
        let outside = Pos2::new(280.0, row(1).y);
        sidebar.frame(vec![Event::PointerMoved(outside)]);
        sidebar.frame(vec![button(outside, false)]);
        assert!(detached(&sidebar.actions).is_empty());
        assert!(sidebar.moves().is_empty());
        assert!(!sidebar.state.dragging_sidebar_row());
    }

    #[test]
    fn losing_pointer_motion_at_the_window_edge_detaches_only_a_held_workspace() {
        let mut sidebar = Fixture::new(&[1, 2]);
        let edge = Pos2::new(1.0, row(1).y);
        sidebar.carry(row(0), edge);
        assert!(detached(&sidebar.actions).is_empty());
        sidebar.frame(vec![Event::PointerGone]);
        assert_eq!(
            detached(&sidebar.actions),
            [(WorkspaceId::new(1), Some(edge))]
        );

        let mut sidebar = Fixture::new(&[1, 2]);
        let middle = Pos2::new(120.0, row(1).y);
        sidebar.carry(row(0), middle);
        sidebar.frame(vec![Event::PointerGone]);
        assert!(detached(&sidebar.actions).is_empty());
    }

    #[test]
    fn a_toggled_sidebar_eases_to_rest_and_reverses_from_where_it_is() {
        let hide = SidebarSlide::toggled(None, true, 10.0);
        assert_eq!(hide.reveal(false, 10.0), Some(1.0));
        let midway = hide.reveal(false, 10.08).unwrap();
        assert!(
            midway > 0.0 && midway < 0.5,
            "ease-out covers most of the way early: {midway}"
        );
        assert_eq!(hide.reveal(false, 10.0 + SIDEBAR_SLIDE), None);

        // Toggled back midway: the slide continues from where the sidebar is.
        let show = SidebarSlide::toggled(Some(hide), false, 10.08);
        assert_eq!(show.reveal(true, 10.08), Some(midway));
        assert!(show.reveal(true, 10.12).unwrap() > midway);
        assert_eq!(show.reveal(true, 10.08 + SIDEBAR_SLIDE), None);

        // A finished slide leaves nothing to continue from.
        assert_eq!(
            SidebarSlide::toggled(Some(hide), false, 11.0).reveal(true, 11.0),
            Some(0.0)
        );
    }

    #[test]
    fn a_carried_terminal_drops_on_another_workspace_row_but_not_its_own() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::platform::fonts::bundled_definitions());
        let config = crate::config::Config::default();
        theme::apply(&ctx, &config);
        let workspaces: Vec<WorkspaceView> = [(1, None), (2, None), (3, Some("me@devbox"))]
            .into_iter()
            .map(|(id, remote): (u64, Option<&str>)| WorkspaceView {
                unread: 0,
                alert: None,
                group: None,
                id: WorkspaceId::new(id),
                name: format!("workspace {id}"),
                cwd: "/srv/app".into(),
                remote: remote.map(str::to_owned),
                branch: None,
                panes: 2,
                running: true,
            })
            .collect();
        let pane = PaneId::new(7);
        let frame = |drag: Option<PaneId>, events: Vec<egui::Event>| {
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    sidebar(
                        ui,
                        Rect::from_min_size(Pos2::ZERO, vec2(216.0, 600.0)),
                        Palette::for_config(&config),
                        &ChromeView {
                            keybindings: &Default::default(),
                            workspaces: &workspaces,
                            groups: &[],
                            sidebar_order: &[
                                SidebarItem::Workspace(WorkspaceId::new(1)),
                                SidebarItem::Workspace(WorkspaceId::new(2)),
                                SidebarItem::Workspace(WorkspaceId::new(3)),
                            ],
                            windows: &[],
                            active: Some(WorkspaceId::new(1)),
                            pane: Some(pane),
                            pane_kind: neptune_model::PaneKind::Terminal,
                            subtitle: "",
                            pull_requests: &[],
                            attached: &[],
                            spawned: &[],
                            worktree: None,
                            ports: &[],
                            pane_generation: 1,
                            zoomed: false,
                            window: Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0)),
                            sidebar: 1.0,
                            sidebar_open: true,
                            sidebar_width: 216.0,
                            sidebar_available: true,
                            pane_drag: drag,
                            panel: None,
                            panel_attention: false,
                        },
                        &mut UiState::default(),
                        &mut actions,
                    );
                },
            );
            output.textures_delta.clear();
            actions
        };
        // Rows sit one step apart below the toolbar strip.
        let own = Pos2::new(100.0, metrics::TOOLBAR_HEIGHT + LIST_TOP + ROW_HEIGHT * 0.5);
        let other = own + vec2(0.0, ROW_STEP);
        let on_a_host = other + vec2(0.0, ROW_STEP);
        let release = |pos| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        };
        let press = |pos| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        };
        frame(None, vec![]);
        for (pos, drag, expected) in [
            (other, Some(pane), Some(WorkspaceId::new(2))),
            (own, Some(pane), None),
            // Its session is local and would not follow it to another machine.
            (on_a_host, Some(pane), None),
            // Without a carried terminal a release is an ordinary click.
            (other, None, None),
        ] {
            frame(drag, vec![egui::Event::PointerMoved(pos), press(pos)]);
            let moved: Vec<_> = frame(drag, vec![release(pos)])
                .into_iter()
                .filter_map(|action| match action {
                    Action::MovePane(pane, Destination::Workspace(workspace)) => {
                        Some((pane, workspace))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                moved,
                expected
                    .map(|workspace| (pane, workspace))
                    .into_iter()
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn workspace_tiles_use_a_visible_initial() {
        assert_eq!(initial("sandbox"), "S");
        assert_eq!(initial("  ~/日本語"), "日");
        assert_eq!(initial("ßeta"), "SS");
        assert_eq!(initial("---"), "·");
    }
