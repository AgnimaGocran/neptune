use crate::{
    FolderId, Layout, PaneId, Project, ProjectId, ProjectSpec, Remote, Window, WindowId,
    WindowSpec, WorkspaceGroupId, WorkspaceId,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UnknownWindow(WindowId),
    WindowLimit,
    InvalidWindows,
    UnknownWorkspace(WorkspaceId),
    UnknownWorkspaceGroup(WorkspaceGroupId),
    UnknownFolder(FolderId),
    WorkspaceGroupLimit,
    UnknownPane(PaneId),
    UnknownSplit(crate::SplitId),
    WorkspaceLimit,
    PaneLimit,
    TotalPaneLimit,
    BrowserLimit,
    InvalidLayout(&'static str),
    InvalidRatio,
    InvalidIdentity,
    InvalidSidebarOrder,
    InvalidName,
    InvalidDirectory,
    InvalidRemote,
    RemoteMismatch,
    IdentityExhausted,
    SpawnLimit,
    SpawnDepth,
    InvalidWorktree,
    UnknownProject(ProjectId),
    ProjectLimit,
    ProjectExists,
    ProjectAgentLimit,
    ProjectPaused,
    InvalidProject,
    LastInView,
    Unanswered,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownWindow(id) => write!(f, "Window {id} does not exist"),
            Self::WindowLimit => f.write_str("Window limit reached"),
            Self::InvalidWindows => f.write_str("Every workspace must belong to exactly one window, with a valid active workspace"),
            Self::UnknownWorkspace(id) => write!(f, "Workspace {id} does not exist"),
            Self::UnknownWorkspaceGroup(id) => write!(f, "Workspace group {id} does not exist"),
            Self::UnknownFolder(id) => write!(f, "Folder {id} does not exist"),
            Self::WorkspaceGroupLimit => f.write_str("Workspace group limit reached"),
            Self::UnknownPane(id) => {
                write!(f, "Pane {id} does not exist in the targeted workspace")
            }
            Self::UnknownSplit(id) => write!(f, "Split {id} does not exist"),
            Self::WorkspaceLimit => f.write_str("Workspace limit reached"),
            Self::PaneLimit => f.write_str("Workspace pane limit reached"),
            Self::TotalPaneLimit => f.write_str("Total pane limit reached"),
            Self::BrowserLimit => f.write_str("Neptune can have at most eight browser panes across its windows"),
            Self::InvalidLayout(reason) => write!(f, "Invalid layout: {reason}"),
            Self::InvalidRatio => f.write_str("Split ratio must be finite and between 0.1 and 0.9"),
            Self::InvalidIdentity => f.write_str("Identities must be nonzero and unique"),
            Self::InvalidSidebarOrder => f.write_str("Sidebar order must contain every group and ungrouped workspace exactly once"),
            Self::InvalidName => f.write_str("Name cannot be empty"),
            Self::InvalidDirectory => f.write_str("Default directory must be an absolute local path"),
            Self::InvalidRemote => f.write_str(
                "SSH host must be a destination such as user@host, without spaces or a leading dash",
            ),
            Self::RemoteMismatch => f.write_str(
                "A terminal keeps its session, so it cannot move between workspaces on different machines",
            ),
            Self::IdentityExhausted => f.write_str("Identity counter exhausted"),
            Self::SpawnLimit => write!(
                f,
                "An agent can have {} agents it started open at once",
                crate::AgentSession::MAX_SPAWNED
            ),
            Self::SpawnDepth => f.write_str(
                "An agent started by an agent that was itself started by one cannot start another",
            ),
            Self::InvalidWorktree => {
                f.write_str("A worktree needs a local workspace, a branch and its own directory")
            }
            Self::UnknownProject(id) => write!(f, "Project {id} does not exist"),
            Self::ProjectLimit => write!(f, "Neptune can have {} projects across its windows", Project::MAX),
            Self::ProjectExists => f.write_str("This workspace already has a project"),
            Self::ProjectAgentLimit => write!(
                f,
                "A project can have {} agents open at once",
                Project::MAX_AGENTS
            ),
            Self::ProjectPaused => f.write_str("The project is paused"),
            Self::InvalidProject => write!(
                f,
                "A project runs in a local workspace, with a name of at most {} characters and Claude Code or Codex as its lead",
                Project::MAX_NAME
            ),
            Self::LastInView => f.write_str("A workspace keeps one terminal in view"),
            Self::Unanswered => f.write_str(
                "Only a terminal an agent or a project started for its own agent runs without a tab",
            ),
        }
    }
}
impl std::error::Error for Error {}

pub(crate) fn validate_default_directory(directory: Option<&Path>) -> Result<(), Error> {
    if let Some(directory) = directory
        && (!directory.is_absolute() || directory.as_os_str().as_encoded_bytes().contains(&0))
    {
        return Err(Error::InvalidDirectory);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub workspaces: usize,
    pub panes_per_workspace: usize,
    pub total_panes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            workspaces: 48,
            panes_per_workspace: 16,
            total_panes: 128,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lifecycle {
    Starting,
    Running,
    Closing,
    Exited,
    Failed(String),
}

/// The content of a layout leaf. Browser navigation and engine state are
/// ephemeral desktop state; restoration opens a fresh browser in this place.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneKind {
    #[default]
    Terminal,
    Browser,
}
impl PaneKind {
    pub const MAX_BROWSERS: usize = 8;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub(crate) id: PaneId,
    pub(crate) kind: PaneKind,
    pub(crate) cwd: PathBuf,
    pub(crate) remote_cwd: Option<PathBuf>,
    pub(crate) agent: Option<crate::AgentSession>,
    pub(crate) pull_requests: Vec<crate::PullRequest>,
    pub(crate) attachments: Vec<crate::Attachment>,
    pub(crate) spawned_by: Option<PaneId>,
    pub(crate) project: Option<ProjectId>,
    pub(crate) worktree: Option<crate::Worktree>,
    pub(crate) generation: u64,
    pub(crate) lifecycle: Lifecycle,
}
impl Pane {
    pub fn kind(&self) -> PaneKind {
        self.kind
    }
    pub fn agent(&self) -> Option<&crate::AgentSession> {
        self.agent.as_ref()
    }
    /// Pull requests the agent linked to this terminal, oldest first.
    pub fn pull_requests(&self) -> &[crate::PullRequest] {
        &self.pull_requests
    }
    /// Files the agent attached to this terminal, oldest first.
    pub fn attachments(&self) -> &[crate::Attachment] {
        &self.attachments
    }
    /// The terminal whose agent started this one's, while both agents last.
    pub fn spawned_by(&self) -> Option<PaneId> {
        self.spawned_by
    }
    /// The project whose lead started this terminal's agent, while the agent
    /// and the project last. Whoever starts the session of a new terminal
    /// reads this to tell a project's agent from any other.
    pub fn project(&self) -> Option<ProjectId> {
        self.project
    }
    /// The git worktree Neptune made for this terminal's agent.
    pub fn worktree(&self) -> Option<&crate::Worktree> {
        self.worktree.as_ref()
    }
    pub fn id(&self) -> PaneId {
        self.id
    }
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }
    /// Last reported directory on the SSH host, independent of the local cwd.
    pub fn remote_cwd(&self) -> Option<&Path> {
        self.remote_cwd.as_deref()
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }
}

/// Construction data; model adoption validates all identity and layout invariants.
#[derive(Debug, Clone)]
pub struct PaneSpec {
    pub id: PaneId,
    pub kind: PaneKind,
    pub cwd: PathBuf,
    pub remote_cwd: Option<PathBuf>,
    pub agent: Option<crate::AgentSession>,
    pub pull_requests: Vec<crate::PullRequest>,
    pub attachments: Vec<crate::Attachment>,
    /// The terminal whose agent started this one's.
    pub spawned_by: Option<PaneId>,
    /// The project whose lead started this terminal's agent.
    pub project: Option<ProjectId>,
    /// The git worktree Neptune made for this terminal's agent.
    pub worktree: Option<crate::Worktree>,
}
#[derive(Debug, Clone)]
pub struct WorkspaceSpec {
    pub id: WorkspaceId,
    pub group: Option<WorkspaceGroupId>,
    /// A pinned workspace keeps a row at the top of the sidebar, apart from
    /// its group or folder.
    pub pinned: bool,
    /// The command this workspace's row is named by, or empty for its name.
    pub command: String,
    pub name: String,
    pub cwd: PathBuf,
    /// An SSH destination; validated when the model adopts the workspace.
    pub remote: Option<String>,
    pub panes: Vec<PaneSpec>,
    pub layout: Layout,
    pub active: PaneId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub(crate) id: WorkspaceId,
    pub(crate) window: WindowId,
    pub(crate) group: Option<WorkspaceGroupId>,
    /// Pinned workspaces stand at the top of the sidebar, in their own rows.
    pub(crate) pinned: bool,
    /// The command the sidebar names this workspace by, empty for its name.
    pub(crate) command: String,
    pub(crate) name: String,
    pub(crate) cwd: PathBuf,
    pub(crate) remote: Option<Remote>,
    pub(crate) panes: Vec<Pane>,
    pub(crate) layout: Layout,
    pub(crate) active: PaneId,
}
impl Workspace {
    pub fn id(&self) -> WorkspaceId {
        self.id
    }
    pub fn window(&self) -> WindowId {
        self.window
    }
    pub fn group(&self) -> Option<WorkspaceGroupId> {
        self.group
    }
    /// Whether the workspace keeps a row at the top of the sidebar.
    pub fn pinned(&self) -> bool {
        self.pinned
    }
    /// The command its row is named by; empty when the name stands instead.
    pub fn command(&self) -> &str {
        &self.command
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The local directory its terminals start in. A remote workspace runs
    /// its SSH client there; each pane tracks its remote directory separately.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }
    /// Set when every terminal of this workspace runs on another machine.
    pub fn remote(&self) -> Option<&Remote> {
        self.remote.as_ref()
    }
    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }
    pub fn layout(&self) -> &Layout {
        &self.layout
    }
    pub fn active(&self) -> PaneId {
        self.active
    }
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.id == id)
    }

    /// A terminal that runs without a tab: one an agent started for another
    /// agent or for a project, until someone opens it, and again once its
    /// tab was put away.
    pub fn is_background(&self, pane: PaneId) -> bool {
        self.pane(pane).is_some() && !self.layout.contains(pane)
    }

    /// Gives a background terminal a tab, after the terminal whose agent
    /// started it or else in the focused place. What is in view stays.
    pub(crate) fn reveal(&mut self, pane: PaneId) -> bool {
        if !self.is_background(pane) {
            return false;
        }
        let target = self
            .pane(pane)
            .and_then(|pane| pane.spawned_by)
            .filter(|parent| self.layout.contains(*parent))
            .unwrap_or(self.active);
        let shown = self.layout.tabs(target).map(|(_, shown)| shown);
        if !self.layout.add_tab(target, pane, None) {
            return false;
        }
        if let Some(shown) = shown {
            self.layout.show(shown);
        }
        true
    }

    /// Takes a pane that is leaving out of the layout. Focus on it passes to
    /// whatever is then in view in its place: the tab that followed it, or the
    /// neighbouring tab group once its own is empty. Background terminals
    /// take the place of the last one in view.
    pub(crate) fn remove_from_layout(&mut self, pane: PaneId) -> Result<(), Error> {
        if !self.layout.contains(pane) {
            return Ok(());
        }
        if self.layout.panes().len() == 1 {
            let background: Vec<PaneId> = self
                .panes
                .iter()
                .map(|item| item.id)
                .filter(|id| *id != pane && !self.layout.contains(*id))
                .collect();
            self.active = pane;
            for id in background {
                self.reveal(id);
            }
        }
        let place = self.layout.shown().iter().position(|id| *id == pane);
        self.layout = self
            .layout
            .clone()
            .remove(pane)
            .ok_or(Error::InvalidLayout("removed every leaf"))?;
        if self.active == pane {
            let shown = self.layout.shown();
            self.active = shown[place.unwrap_or(0).min(shown.len() - 1)];
        }
        Ok(())
    }
}

/// Construction data for a sidebar folder. Groups contain workspaces, never groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGroupSpec {
    pub id: WorkspaceGroupId,
    pub name: String,
    pub collapsed: bool,
    pub default_directory: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGroup {
    pub(crate) id: WorkspaceGroupId,
    pub(crate) name: String,
    pub(crate) collapsed: bool,
    pub(crate) default_directory: Option<PathBuf>,
}
impl WorkspaceGroup {
    pub fn id(&self) -> WorkspaceGroupId {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn collapsed(&self) -> bool {
        self.collapsed
    }
    /// Starting directory for new local workspaces in this group. Terminals
    /// within each workspace continue to inherit their source pane's directory.
    pub fn default_directory(&self) -> Option<&Path> {
        self.default_directory.as_deref()
    }
}

/// A top-level sidebar entry. Group and folder children are ordered by
/// `Model::workspaces`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarItem {
    Workspace(WorkspaceId),
    Group(WorkspaceGroupId),
    /// A directory more than one ungrouped workspace starts in.
    Folder(FolderId),
}

/// A directory the sidebar shows as a branch over the workspaces started
/// there. It is not a group: nobody names it, it disappears with its last
/// workspace, and moving one moves the workspaces it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
    pub(crate) id: FolderId,
    pub(crate) path: PathBuf,
    pub(crate) collapsed: bool,
    /// False once the sidebar was told to keep this directory's workspaces on
    /// rows of their own. The folder is retained so the choice survives.
    pub(crate) grouped: bool,
}

impl Folder {
    pub fn id(&self) -> FolderId {
        self.id
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn collapsed(&self) -> bool {
        self.collapsed
    }
    pub fn grouped(&self) -> bool {
        self.grouped
    }
}

/// A folder as it is saved and restored. Only the collapse state and the
/// identity the sidebar order names are kept; the path derives both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSpec {
    pub id: FolderId,
    pub path: PathBuf,
    pub collapsed: bool,
    /// Whether the directory still shows as one row for its workspaces.
    pub grouped: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub(crate) workspaces: Vec<Workspace>,
    pub(crate) groups: Vec<WorkspaceGroup>,
    pub(crate) folders: Vec<Folder>,
    pub(crate) sidebar_order: Vec<SidebarItem>,
    pub(crate) projects: Vec<Project>,
    pub(crate) active: Option<WorkspaceId>,
    pub(crate) sidebar: bool,
    pub(crate) windows: Vec<Window>,
    pub(crate) current_window: WindowId,
    pub(crate) next_window: u64,
    pub(crate) next_workspace: u64,
    pub(crate) next_group: u64,
    pub(crate) next_folder: u64,
    pub(crate) next_pane: u64,
    pub(crate) next_split: u64,
    pub(crate) next_project: u64,
    pub(crate) limits: Limits,
    /// Oldest first.
    /// The workspaces shown as compact tabs under another, child first.
    pub(crate) minis: Vec<(WorkspaceId, WorkspaceId)>,
    pub(crate) conversations: Vec<crate::Conversation>,
}
impl Default for Model {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}
impl Model {
    pub fn new(limits: Limits) -> Self {
        Self {
            workspaces: Vec::new(),
            groups: Vec::new(),
            folders: Vec::new(),
            sidebar_order: Vec::new(),
            projects: Vec::new(),
            active: None,
            sidebar: true,
            windows: vec![Window {
                id: WindowId::ROOT,
                active: None,
                sidebar: true,
            }],
            current_window: WindowId::ROOT,
            next_window: 2,
            next_workspace: 1,
            next_group: 1,
            next_folder: 1,
            next_pane: 1,
            next_split: 1,
            next_project: 1,
            limits,
            minis: Vec::new(),
            conversations: Vec::new(),
        }
    }
    /// What agents linked and attached in conversations no tab shows now,
    /// oldest first.
    pub fn conversations(&self) -> &[crate::Conversation] {
        &self.conversations
    }
    /// A restored model takes back the conversations saved with it; the
    /// newest are kept, and one that is not well formed is left out.
    pub fn with_conversations(mut self, mut conversations: Vec<crate::Conversation>) -> Self {
        conversations.retain(crate::Conversation::is_valid);
        let extra = conversations.len().saturating_sub(crate::Conversation::MAX);
        conversations.drain(..extra);
        self.conversations = conversations;
        self
    }
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }
    pub fn windows(&self) -> &[Window] {
        &self.windows
    }
    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|window| window.id == id)
    }
    pub fn current_window(&self) -> WindowId {
        self.current_window
    }

    /// Adopts window organization after the normal workspace restore boundary.
    /// Omission preserves the historical single-window organization.
    pub fn with_windows(mut self, specs: Vec<WindowSpec>) -> Result<Self, Error> {
        if specs.is_empty() || !specs.iter().any(|spec| spec.id == WindowId::ROOT) {
            return Err(Error::InvalidWindows);
        }
        if specs.len() > self.workspaces.len().saturating_add(1) {
            return Err(Error::WindowLimit);
        }
        let mut ids = HashSet::new();
        let mut assigned = HashSet::new();
        let mut next_window = 2;
        for spec in &specs {
            if spec.id.get() == 0 || !ids.insert(spec.id) {
                return Err(Error::InvalidIdentity);
            }
            next_window = next_window.max(
                spec.id
                    .get()
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?,
            );
            if spec.id != WindowId::ROOT && spec.workspaces.is_empty() {
                return Err(Error::InvalidWindows);
            }
            for workspace in &spec.workspaces {
                if self.workspace(*workspace).is_none() {
                    return Err(Error::UnknownWorkspace(*workspace));
                }
                if !assigned.insert(*workspace) {
                    return Err(Error::InvalidWindows);
                }
            }
            if spec.active.is_some_and(|id| !spec.workspaces.contains(&id)) {
                return Err(Error::InvalidWindows);
            }
        }
        if assigned.len() != self.workspaces.len() {
            return Err(Error::InvalidWindows);
        }
        self.windows.clear();
        for spec in specs {
            for id in &spec.workspaces {
                self.workspace_mut(*id)?.window = spec.id;
            }
            self.windows.push(Window {
                id: spec.id,
                active: spec.active.or_else(|| spec.workspaces.first().copied()),
                sidebar: spec.sidebar,
            });
        }
        self.next_window = next_window;
        self.current_window = WindowId::ROOT;
        if let Some((active, sidebar)) = self
            .window(WindowId::ROOT)
            .map(|root| (root.active, root.sidebar))
        {
            self.active = active;
            self.sidebar = sidebar;
        }
        Ok(self)
    }

    pub fn window_specs(&self) -> Vec<WindowSpec> {
        self.windows
            .iter()
            .map(|window| WindowSpec {
                id: window.id,
                workspaces: self
                    .workspaces
                    .iter()
                    .filter(|workspace| workspace.window == window.id)
                    .map(Workspace::id)
                    .collect(),
                active: window.active,
                sidebar: window.sidebar,
            })
            .collect()
    }

    pub(crate) fn set_window_context(&mut self, id: WindowId) -> Result<(), Error> {
        let window = self.window(id).ok_or(Error::UnknownWindow(id))?;
        let active = window.active;
        let sidebar = window.sidebar;
        self.current_window = id;
        self.active = active;
        self.sidebar = sidebar;
        Ok(())
    }

    /// Selection belongs to the target's window. Background work must never
    /// retarget another native window's terminal input.
    pub(crate) fn select_workspace(&mut self, id: WorkspaceId) -> Result<(), Error> {
        let owner = self
            .workspace(id)
            .ok_or(Error::UnknownWorkspace(id))?
            .window;
        if let Some(window) = self.windows.iter_mut().find(|window| window.id == owner) {
            window.active = Some(id);
        }
        if owner == self.current_window {
            self.active = Some(id);
        }
        Ok(())
    }

    pub(crate) fn sync_windows(&mut self) {
        if let Some(current) = self
            .windows
            .iter_mut()
            .find(|window| window.id == self.current_window)
        {
            current.active = self.active;
            current.sidebar = self.sidebar;
        }
        for window in &mut self.windows {
            if !window.active.is_some_and(|id| {
                self.workspaces
                    .iter()
                    .any(|workspace| workspace.id == id && workspace.window == window.id)
            }) {
                window.active = self
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.window == window.id)
                    .map(Workspace::id);
            }
        }
        self.windows.retain(|window| {
            window.id == WindowId::ROOT
                || self
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.window == window.id)
        });
        if self.window(self.current_window).is_none() {
            self.current_window = WindowId::ROOT;
        }
        if let Some((active, sidebar)) = self
            .window(self.current_window)
            .map(|current| (current.active, current.sidebar))
        {
            self.active = active;
            self.sidebar = sidebar;
        }
    }
    pub fn groups(&self) -> &[WorkspaceGroup] {
        &self.groups
    }
    pub fn sidebar_order(&self) -> &[SidebarItem] {
        &self.sidebar_order
    }
    pub fn group(&self, id: WorkspaceGroupId) -> Option<&WorkspaceGroup> {
        self.groups.iter().find(|group| group.id == id)
    }
    /// The folders the sidebar shows, in the order it shows them.
    pub fn folders(&self) -> &[Folder] {
        &self.folders
    }
    pub fn folder(&self, id: FolderId) -> Option<&Folder> {
        self.folders.iter().find(|folder| folder.id == id)
    }
    /// The folder that holds the workspace's row, if any.
    pub fn folder_of(&self, workspace: WorkspaceId) -> Option<FolderId> {
        let workspace = self.workspace(workspace)?;
        if workspace.pinned || workspace.group.is_some() {
            return None;
        }
        self.folders
            .iter()
            .find(|folder| workspace.cwd == folder.path && folder.grouped)
            .map(Folder::id)
    }
    /// The workspace whose compact tabs hold this one, if any.
    pub fn mini_parent(&self, workspace: WorkspaceId) -> Option<WorkspaceId> {
        self.minis
            .iter()
            .find(|(child, _)| *child == workspace)
            .map(|(_, parent)| *parent)
    }
    /// The workspaces this one lists as compact tabs, in sidebar order.
    pub fn mini_tabs(&self, workspace: WorkspaceId) -> impl Iterator<Item = &Workspace> {
        self.minis
            .iter()
            .filter(move |(_, parent)| *parent == workspace)
            .filter_map(|(child, _)| self.workspace(*child))
    }
    /// The workspaces a folder holds, in sidebar order.
    pub fn folder_workspaces(&self, id: FolderId) -> impl Iterator<Item = &Workspace> {
        let path = self.folder(id).map(|folder| folder.path.clone());
        self.workspaces.iter().filter(move |workspace| {
            workspace.group.is_none() && !workspace.pinned && Some(&workspace.cwd) == path.as_ref()
        })
    }
    /// The top-level row a workspace stands in: its own pinned row, its
    /// group, its folder, or its own row.
    pub fn entry_of(&self, id: WorkspaceId) -> Option<SidebarItem> {
        let workspace = self.workspace(id)?;
        Some(if workspace.pinned {
            SidebarItem::Workspace(id)
        } else {
            match workspace.group {
                Some(group) => SidebarItem::Group(group),
                None => self
                    .folders
                    .iter()
                    .find(|folder| folder.path == workspace.cwd && folder.grouped)
                    .map_or(SidebarItem::Workspace(id), |folder| {
                        SidebarItem::Folder(folder.id)
                    }),
            }
        })
    }
    pub fn workspace(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.iter().find(|workspace| workspace.id == id)
    }
    pub fn active_workspace(&self) -> Option<WorkspaceId> {
        self.active
    }
    pub fn active_pane(&self) -> Option<PaneId> {
        self.active
            .and_then(|id| self.workspace(id))
            .map(|workspace| workspace.active)
    }
    pub fn sidebar(&self) -> bool {
        self.sidebar
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    /// Replace the limits the model enforces. What already exists stays;
    /// the new limits bound what is opened from here on.
    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.workspaces
            .iter()
            .find_map(|workspace| workspace.pane(id))
    }
    /// The terminals whose agents the agent in `parent` started, oldest first.
    pub fn spawned(&self, parent: PaneId) -> impl Iterator<Item = &Pane> {
        self.workspaces
            .iter()
            .flat_map(|workspace| &workspace.panes)
            .filter(move |pane| pane.spawned_by == Some(parent))
    }
    /// The projects of this window, oldest first.
    pub fn projects(&self) -> &[Project] {
        &self.projects
    }
    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }
    /// A workspace has at most one project.
    pub fn project_of(&self, workspace: WorkspaceId) -> Option<&Project> {
        self.projects
            .iter()
            .find(|project| project.workspace == workspace)
    }
    /// The terminals whose agents the lead of `project` started and that
    /// still run them, oldest first.
    pub fn project_agents(&self, project: ProjectId) -> impl Iterator<Item = &Pane> {
        self.workspaces
            .iter()
            .flat_map(|workspace| &workspace.panes)
            .filter(move |pane| pane.project == Some(project))
    }
    /// Whether the tab of `pane` may be put away while its terminal runs on.
    /// A workspace keeps one terminal in view, and a terminal without a tab
    /// is found again only where its project or the agent that started it
    /// lists it.
    pub fn can_background(&self, pane: PaneId) -> Result<(), Error> {
        let workspace = self
            .workspace_for_pane(pane)
            .and_then(|id| self.workspace(id))
            .filter(|workspace| !workspace.is_background(pane))
            .ok_or(Error::UnknownPane(pane))?;
        if workspace.layout.panes().len() == 1 {
            return Err(Error::LastInView);
        }
        let listed = workspace.pane(pane).is_some_and(|item| {
            item.project.is_some()
                || item
                    .spawned_by
                    .is_some_and(|parent| workspace.pane(parent).is_some())
        });
        if listed {
            Ok(())
        } else {
            Err(Error::Unanswered)
        }
    }
    /// How many agents stand between `pane` and one a person started. A chain
    /// longer than the limit, as a loop would be, counts as past it. A
    /// project's lead counts as an agent a person started, so its agents can
    /// start agents and those cannot.
    pub(crate) fn spawn_depth(&self, pane: PaneId) -> usize {
        let mut depth = 0;
        let mut next = self.pane(pane);
        while let Some(item) = next {
            let Some(parent) = item.spawned_by else {
                depth += usize::from(item.project.is_some());
                break;
            };
            depth += 1;
            if depth > crate::AgentSession::MAX_SPAWN_DEPTH {
                break;
            }
            next = self.pane(parent);
        }
        depth
    }
    /// Ends every link to a terminal that is gone or no longer runs an agent,
    /// every project whose workspace is gone, and every link to a project
    /// that is gone or belongs to another workspace. A background terminal
    /// whose link ended gets a tab: nothing runs out of view that no agent
    /// or project answers for.
    pub(crate) fn release_spawned(&mut self) -> bool {
        let projects = self.projects.len();
        self.projects.retain(|project| {
            self.workspaces
                .iter()
                .any(|workspace| workspace.id == project.workspace)
        });
        let strays: Vec<PaneId> = self
            .workspaces
            .iter()
            .flat_map(|workspace| {
                workspace.panes.iter().filter(|pane| {
                    pane.project.is_some_and(|project| {
                        self.project(project)
                            .is_none_or(|project| project.workspace != workspace.id)
                    })
                })
            })
            .map(|pane| pane.id)
            .collect();
        for pane in &strays {
            if let Ok(pane) = self.pane_mut(*pane) {
                pane.project = None;
            }
        }
        let orphans: Vec<PaneId> = self
            .workspaces
            .iter()
            .flat_map(|workspace| &workspace.panes)
            .filter(|pane| {
                pane.spawned_by.is_some_and(|parent| {
                    self.pane(parent)
                        .is_none_or(|parent| parent.agent.is_none())
                })
            })
            .map(|pane| pane.id)
            .collect();
        for pane in &orphans {
            if let Ok(pane) = self.pane_mut(*pane) {
                pane.spawned_by = None;
            }
        }
        let mut changed =
            !orphans.is_empty() || !strays.is_empty() || self.projects.len() != projects;
        for workspace in &mut self.workspaces {
            let unlinked: Vec<PaneId> = workspace
                .panes
                .iter()
                .filter(|pane| pane.spawned_by.is_none() && pane.project.is_none())
                .map(|pane| pane.id)
                .collect();
            for pane in unlinked {
                changed |= workspace.reveal(pane);
            }
        }
        changed
    }
    pub fn pane_count(&self) -> usize {
        self.workspaces
            .iter()
            .map(|workspace| workspace.panes.len())
            .sum()
    }
    pub fn workspace_for_pane(&self, id: PaneId) -> Option<WorkspaceId> {
        self.workspaces
            .iter()
            .find(|workspace| workspace.pane(id).is_some())
            .map(|workspace| workspace.id)
    }

    /// Filesystem checks are deliberately left to the persistence/runtime owner.
    /// Validation is atomic: a rejected restore never partially changes a model.
    pub fn restore(
        specs: Vec<WorkspaceSpec>,
        active: Option<WorkspaceId>,
        sidebar: bool,
        limits: Limits,
    ) -> Result<Self, Error> {
        Self::restore_grouped(specs, Vec::new(), Vec::new(), active, sidebar, limits)
    }

    /// Restores projects along with everything `restore_ordered` does. This
    /// is the only entry that accepts a terminal's link to a project: the
    /// others restore no project, so they reject such a terminal.
    pub fn restore_with_projects(
        specs: Vec<WorkspaceSpec>,
        groups: Vec<WorkspaceGroupSpec>,
        folders: Vec<FolderSpec>,
        order: Option<Vec<SidebarItem>>,
        projects: Vec<ProjectSpec>,
        active: Option<WorkspaceId>,
        sidebar: bool,
        limits: Limits,
    ) -> Result<Self, Error> {
        Self::restore_all(specs, groups, folders, projects, active, sidebar, limits)?
            .ordered(order, active)
    }

    /// Restores organization along with workspaces, validating membership atomically.
    /// Empty groups are retained; their count shares the workspace resource limit.
    pub fn restore_grouped(
        specs: Vec<WorkspaceSpec>,
        groups: Vec<WorkspaceGroupSpec>,
        folders: Vec<FolderSpec>,
        active: Option<WorkspaceId>,
        sidebar: bool,
        limits: Limits,
    ) -> Result<Self, Error> {
        Self::restore_all(specs, groups, folders, Vec::new(), active, sidebar, limits)
    }

    fn restore_all(
        specs: Vec<WorkspaceSpec>,
        groups: Vec<WorkspaceGroupSpec>,
        folders: Vec<FolderSpec>,
        projects: Vec<ProjectSpec>,
        active: Option<WorkspaceId>,
        sidebar: bool,
        limits: Limits,
    ) -> Result<Self, Error> {
        if specs.len() > limits.workspaces {
            return Err(Error::WorkspaceLimit);
        }
        if specs
            .iter()
            .flat_map(|w| &w.panes)
            .filter(|p| p.kind == PaneKind::Browser)
            .count()
            > PaneKind::MAX_BROWSERS
        {
            return Err(Error::BrowserLimit);
        }
        let mut model = Self::new(limits);
        if groups.len() > limits.workspaces {
            return Err(Error::WorkspaceGroupLimit);
        }
        let mut group_ids = HashSet::new();
        for group in groups {
            if group.id.get() == 0 || !group_ids.insert(group.id) {
                return Err(Error::InvalidIdentity);
            }
            if group.name.trim().is_empty() {
                return Err(Error::InvalidName);
            }
            validate_default_directory(group.default_directory.as_deref())?;
            model.next_group = model.next_group.max(
                group
                    .id
                    .get()
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?,
            );
            model.groups.push(WorkspaceGroup {
                id: group.id,
                name: group.name,
                collapsed: group.collapsed,
                default_directory: group.default_directory,
            });
        }
        // Folders follow the workspaces they hold; a saved one keeps its
        // identity and whether it was closed.
        if folders.len() > limits.workspaces {
            return Err(Error::WorkspaceGroupLimit);
        }
        let mut folder_ids = HashSet::new();
        for folder in folders {
            if folder.id.get() == 0
                || !folder_ids.insert(folder.id)
                || model.folders.iter().any(|held| held.path == folder.path)
            {
                return Err(Error::InvalidIdentity);
            }
            model.next_folder = model.next_folder.max(
                folder
                    .id
                    .get()
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?,
            );
            model.folders.push(Folder {
                id: folder.id,
                path: folder.path,
                collapsed: folder.collapsed,
                grouped: folder.grouped,
            });
        }
        let mut workspace_ids = HashSet::new();
        let mut pane_ids = HashSet::new();
        let mut split_ids = HashSet::new();
        for spec in specs {
            if let Some(group) = spec.group
                && !group_ids.contains(&group)
            {
                return Err(Error::UnknownWorkspaceGroup(group));
            }
            if spec.id.get() == 0 || !workspace_ids.insert(spec.id) {
                return Err(Error::InvalidIdentity);
            }
            if spec.name.trim().is_empty() {
                return Err(Error::InvalidName);
            }
            let remote = spec.remote.as_deref().map(Remote::parse).transpose()?;
            if spec.panes.is_empty() {
                return Err(Error::InvalidLayout("workspace has no panes"));
            }
            if spec.panes.len() > limits.panes_per_workspace {
                return Err(Error::PaneLimit);
            }
            let mut members = HashSet::new();
            for pane in &spec.panes {
                if pane.kind == PaneKind::Browser
                    && (pane.agent.is_some()
                        || pane.remote_cwd.is_some()
                        || pane.spawned_by.is_some()
                        || pane.project.is_some()
                        || pane.worktree.is_some())
                {
                    return Err(Error::InvalidLayout("browser has terminal state"));
                }
                if pane
                    .agent
                    .as_ref()
                    .is_some_and(|agent| !agent.is_valid() || remote.is_some())
                {
                    return Err(Error::InvalidLayout("invalid agent resume reference"));
                }
                if pane.pull_requests.len() > crate::PullRequest::MAX_PER_PANE
                    || (pane.agent.is_none() && !pane.pull_requests.is_empty())
                {
                    return Err(Error::InvalidLayout("invalid pull request links"));
                }
                if pane
                    .worktree
                    .as_ref()
                    .is_some_and(|worktree| !worktree.is_valid() || remote.is_some())
                {
                    return Err(Error::InvalidWorktree);
                }
                if pane.attachments.len() > crate::Attachment::MAX_PER_PANE
                    || (pane.agent.is_none() && !pane.attachments.is_empty())
                {
                    return Err(Error::InvalidLayout("invalid attachments"));
                }
                if remote.is_none() && pane.remote_cwd.is_some() {
                    return Err(Error::InvalidLayout("local pane has a remote directory"));
                }
                if pane.id.get() == 0 || !pane_ids.insert(pane.id) {
                    return Err(Error::InvalidIdentity);
                }
                members.insert(pane.id);
                model.next_pane = model.next_pane.max(
                    pane.id
                        .get()
                        .checked_add(1)
                        .ok_or(Error::IdentityExhausted)?,
                );
            }
            // Only a terminal an agent started for another, or for a project,
            // runs without a tab.
            let placed: HashSet<PaneId> = spec.layout.panes().into_iter().collect();
            for pane in &spec.panes {
                if (pane.spawned_by.is_some() || pane.project.is_some())
                    && !placed.contains(&pane.id)
                {
                    members.remove(&pane.id);
                }
            }
            if !members.contains(&spec.active) {
                return Err(Error::UnknownPane(spec.active));
            }
            spec.layout.validate(&members, &mut split_ids)?;
            // The focused pane is always the tab in view in its place.
            let mut layout = spec.layout;
            layout.show(spec.active);
            model.next_split = model.next_split.max(
                layout
                    .max_split_id()
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?,
            );
            model.next_workspace = model.next_workspace.max(
                spec.id
                    .get()
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?,
            );
            model.workspaces.push(Workspace {
                id: spec.id,
                window: WindowId::ROOT,
                group: spec.group,
                pinned: spec.pinned,
                command: spec.command,
                name: spec.name,
                cwd: spec.cwd,
                remote,
                panes: spec
                    .panes
                    .into_iter()
                    .map(|pane| Pane {
                        id: pane.id,
                        kind: pane.kind,
                        cwd: pane.cwd,
                        remote_cwd: pane.remote_cwd,
                        agent: pane.agent,
                        pull_requests: pane.pull_requests,
                        attachments: pane.attachments,
                        spawned_by: pane.spawned_by,
                        project: pane.project,
                        worktree: pane.worktree,
                        generation: 1,
                        lifecycle: Lifecycle::Starting,
                    })
                    .collect(),
                layout,
                active: spec.active,
            });
        }
        if model.pane_count() > limits.total_panes {
            return Err(Error::TotalPaneLimit);
        }
        // An agent is another's only while both can be resumed, and the
        // chain ends: no terminal is its own ancestor.
        for pane in model
            .workspaces
            .iter()
            .flat_map(|workspace| &workspace.panes)
        {
            let Some(parent) = pane.spawned_by else {
                continue;
            };
            if pane.agent.is_none()
                || model
                    .pane(parent)
                    .is_none_or(|parent| parent.agent.is_none())
                || model.spawn_depth(pane.id) > crate::AgentSession::MAX_SPAWN_DEPTH
                || model.spawned(parent).count() > crate::AgentSession::MAX_SPAWNED
            {
                return Err(Error::InvalidLayout("invalid spawned agent"));
            }
        }
        if projects.len() > Project::MAX {
            return Err(Error::ProjectLimit);
        }
        for spec in projects {
            if spec.id.get() == 0
                || model
                    .projects
                    .iter()
                    .any(|project| project.id == spec.id || project.key == spec.key)
            {
                return Err(Error::InvalidIdentity);
            }
            Project::validate(&spec.name, spec.lead)?;
            let workspace = model
                .workspace(spec.workspace)
                .ok_or(Error::UnknownWorkspace(spec.workspace))?;
            if workspace.remote.is_some() {
                return Err(Error::InvalidProject);
            }
            if model.project_of(spec.workspace).is_some() {
                return Err(Error::ProjectExists);
            }
            model.next_project = model.next_project.max(
                spec.id
                    .get()
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?,
            );
            model.projects.push(Project {
                id: spec.id,
                key: spec.key,
                name: spec.name,
                workspace: spec.workspace,
                lead: spec.lead,
                paused: spec.paused,
            });
        }
        // A project's agent is a resumable one in the project's workspace
        // that no other agent started.
        for workspace in &model.workspaces {
            for pane in &workspace.panes {
                let Some(id) = pane.project else {
                    continue;
                };
                let project = model.project(id).ok_or(Error::UnknownProject(id))?;
                if project.workspace != workspace.id
                    || pane.agent.is_none()
                    || pane.spawned_by.is_some()
                {
                    return Err(Error::InvalidLayout("invalid project agent"));
                }
            }
        }
        if model
            .projects
            .iter()
            .any(|project| model.project_agents(project.id).count() > Project::MAX_AGENTS)
        {
            return Err(Error::ProjectAgentLimit);
        }
        if let Some(id) = active
            && model.workspace(id).is_none()
        {
            return Err(Error::UnknownWorkspace(id));
        }
        model.order_workspaces();
        model.active = active.or_else(|| model.workspaces.first().map(Workspace::id));
        model.sidebar = sidebar;
        model.sync_windows();
        Ok(model)
    }

    pub fn specs(&self) -> Vec<WorkspaceSpec> {
        self.workspaces
            .iter()
            .map(|workspace| WorkspaceSpec {
                id: workspace.id,
                group: workspace.group,
                pinned: workspace.pinned,
                command: workspace.command.clone(),
                name: workspace.name.clone(),
                cwd: workspace.cwd.clone(),
                remote: workspace
                    .remote
                    .as_ref()
                    .map(|remote| remote.destination().to_owned()),
                panes: workspace
                    .panes
                    .iter()
                    .map(|pane| PaneSpec {
                        id: pane.id,
                        kind: pane.kind,
                        cwd: pane.cwd.clone(),
                        remote_cwd: pane.remote_cwd.clone(),
                        agent: pane.agent.clone(),
                        pull_requests: pane.pull_requests.clone(),
                        attachments: pane.attachments.clone(),
                        spawned_by: pane.spawned_by,
                        project: pane.project,
                        worktree: pane.worktree.clone(),
                    })
                    .collect(),
                layout: workspace.layout.clone(),
                active: workspace.active,
            })
            .collect()
    }

    /// Restores a mixed top-level order, or the historical root-first order
    /// when loading a format that predates sidebar ordering.
    pub fn restore_ordered(
        specs: Vec<WorkspaceSpec>,
        groups: Vec<WorkspaceGroupSpec>,
        folders: Vec<FolderSpec>,
        order: Option<Vec<SidebarItem>>,
        active: Option<WorkspaceId>,
        sidebar: bool,
        limits: Limits,
    ) -> Result<Self, Error> {
        Self::restore_grouped(specs, groups, folders, active, sidebar, limits)?
            .ordered(order, active)
    }

    fn ordered(
        mut self,
        order: Option<Vec<SidebarItem>>,
        active: Option<WorkspaceId>,
    ) -> Result<Self, Error> {
        if let Some(order) = order {
            // A folder stands for the workspaces it holds: a save from before
            // folders lists them, and one from after lists the folder.
            let expand =
                |model: &Self, item: SidebarItem, into: &mut HashSet<SidebarItem>| match item {
                    SidebarItem::Folder(id) => into.extend(
                        model
                            .folder_workspaces(id)
                            .map(|workspace| SidebarItem::Workspace(workspace.id)),
                    ),
                    item => {
                        into.insert(item);
                    }
                };
            let mut saved = HashSet::new();
            for item in order.iter().copied() {
                expand(&self, item, &mut saved);
            }
            let mut expected = HashSet::new();
            for item in self.sidebar_order.iter().copied() {
                expand(&self, item, &mut expected);
            }
            if saved != expected
                || order.iter().copied().collect::<HashSet<_>>().len() != order.len()
            {
                return Err(Error::InvalidSidebarOrder);
            }
            self.sidebar_order = order;
            self.order_workspaces();
            self.active = active.or_else(|| self.workspaces.first().map(Workspace::id));
            self.sync_windows();
        }
        Ok(self)
    }

    pub fn project_specs(&self) -> Vec<ProjectSpec> {
        self.projects
            .iter()
            .map(|project| ProjectSpec {
                id: project.id,
                key: project.key.clone(),
                name: project.name.clone(),
                workspace: project.workspace,
                lead: project.lead,
                paused: project.paused,
            })
            .collect()
    }

    pub fn group_specs(&self) -> Vec<WorkspaceGroupSpec> {
        self.groups
            .iter()
            .map(|group| WorkspaceGroupSpec {
                id: group.id,
                name: group.name.clone(),
                collapsed: group.collapsed,
                default_directory: group.default_directory.clone(),
            })
            .collect()
    }

    /// The folders as they are saved: only what the sidebar cannot derive.
    pub fn folder_specs(&self) -> Vec<FolderSpec> {
        self.folders
            .iter()
            .map(|folder| FolderSpec {
                id: folder.id,
                path: folder.path.clone(),
                collapsed: folder.collapsed,
                grouped: folder.grouped,
            })
            .collect()
    }

    pub(crate) fn folder_mut(&mut self, id: FolderId) -> Result<&mut Folder, Error> {
        self.folders
            .iter_mut()
            .find(|folder| folder.id == id)
            .ok_or(Error::UnknownFolder(id))
    }

    pub(crate) fn group_mut(&mut self, id: WorkspaceGroupId) -> Result<&mut WorkspaceGroup, Error> {
        self.groups
            .iter_mut()
            .find(|group| group.id == id)
            .ok_or(Error::UnknownWorkspaceGroup(id))
    }
    /// Takes the sidebar's top-level rows as the caller orders them. Every row
    /// names something that exists, and each stands once; the model then
    /// settles the rows it links, including pinned rows at the top.
    pub(crate) fn set_sidebar_order(&mut self, order: Vec<SidebarItem>) -> Result<(), Error> {
        let mut seen = HashSet::new();
        for item in &order {
            match item {
                SidebarItem::Workspace(id) if self.workspace(*id).is_none() => {
                    return Err(Error::UnknownWorkspace(*id));
                }
                SidebarItem::Group(id) if self.group(*id).is_none() => {
                    return Err(Error::UnknownWorkspaceGroup(*id));
                }
                SidebarItem::Folder(id) if self.folder(*id).is_none() => {
                    return Err(Error::UnknownFolder(*id));
                }
                _ => {}
            }
            if !seen.insert(*item) {
                return Err(Error::InvalidSidebarOrder);
            }
        }
        self.sidebar_order = order;
        self.order_workspaces();
        Ok(())
    }

    /// Reconcile created/closed entries and keep navigation identical to the
    /// sidebar, including children of collapsed groups and folders. Stable
    /// sorting retains the order within a group; terminal/session state is
    /// untouched.
    pub(crate) fn order_workspaces(&mut self) {
        // Workspaces started in one directory share a folder; one alone in
        // its directory keeps a plain row.
        let mut held: Vec<(PathBuf, usize)> = Vec::new();
        for workspace in self
            .workspaces
            .iter()
            .filter(|w| w.group.is_none() && !w.pinned)
        {
            match held.iter_mut().find(|(path, _)| *path == workspace.cwd) {
                Some((_, count)) => *count += 1,
                None => held.push((workspace.cwd.clone(), 1)),
            }
        }
        held.retain(|(_, count)| *count > 1);
        // A folder stays while it holds a branch, or because the sidebar was
        // told to keep that directory's workspaces apart.
        self.folders
            .retain(|folder| !folder.grouped || held.iter().any(|(path, _)| *path == folder.path));
        for (path, _) in &held {
            if self.folders.iter().all(|folder| folder.path != *path) {
                self.folders.push(Folder {
                    id: FolderId::new(self.next_folder),
                    path: path.clone(),
                    collapsed: false,
                    grouped: true,
                });
                self.next_folder += 1;
            }
        }
        // A later workspace in the same directory joins the first one as one of
        // its compact tabs and leaves the top-level rows; folders keep their own
        // grouping untouched.
        self.minis.clear();
        let mut first_of: Vec<(std::path::PathBuf, WorkspaceId)> = Vec::new();
        for workspace in &self.workspaces {
            if workspace.pinned
                || workspace.group.is_some()
                || self
                    .folders
                    .iter()
                    .any(|folder| folder.path == workspace.cwd && folder.grouped)
            {
                continue;
            }
            match first_of.iter().find(|(cwd, _)| *cwd == workspace.cwd) {
                Some((_, parent)) => self.minis.push((workspace.id, *parent)),
                None => first_of.push((workspace.cwd.clone(), workspace.id)),
            }
        }
        // The row each workspace belongs to: its group, its folder, or itself.
        let entries: Vec<(WorkspaceId, SidebarItem)> = self
            .workspaces
            .iter()
            .filter(|workspace| !self.minis.iter().any(|(child, _)| *child == workspace.id))
            .map(|workspace| {
                let entry = if workspace.pinned {
                    SidebarItem::Workspace(workspace.id)
                } else {
                    match workspace.group {
                        Some(group) => SidebarItem::Group(group),
                        None => self
                            .folders
                            .iter()
                            .find(|folder| folder.path == workspace.cwd && folder.grouped)
                            .map_or(SidebarItem::Workspace(workspace.id), |folder| {
                                SidebarItem::Folder(folder.id)
                            }),
                    }
                };
                (workspace.id, entry)
            })
            .collect();
        // A workspace that joins a folder gives its place to that folder, and
        // the ones that follow it are no longer top-level entries.
        let mut order: Vec<SidebarItem> = Vec::new();
        let saved = std::mem::take(&mut self.sidebar_order);
        // A pinned row returning to its group leaves that group's own row
        // where it stands rather than taking the pinned slot.
        let standing: HashSet<SidebarItem> = saved
            .iter()
            .copied()
            .filter(|item| !matches!(item, SidebarItem::Workspace(_)))
            .collect();
        for item in saved {
            let keep = match item {
                SidebarItem::Workspace(id) => entries
                    .iter()
                    .find(|(workspace, _)| *workspace == id)
                    .map(|(_, entry)| *entry),
                SidebarItem::Group(id) => self
                    .groups
                    .iter()
                    .any(|group| group.id == id)
                    .then_some(item),
                SidebarItem::Folder(id) => self
                    .folders
                    .iter()
                    .any(|folder| folder.id == id && folder.grouped)
                    .then_some(item),
            };
            if let Some(keep) = keep
                && !order.contains(&keep)
                && (keep == item || !standing.contains(&keep))
            {
                order.push(keep);
            }
        }
        // Entries the order does not have yet: plain workspaces follow the
        // last one of them, groups and folders follow everything.
        for (_, entry) in &entries {
            if order.contains(entry) {
                continue;
            }
            match entry {
                SidebarItem::Workspace(_) => {
                    let index = order
                        .iter()
                        .rposition(|item| matches!(item, SidebarItem::Workspace(_)))
                        .map_or(0, |index| index + 1);
                    order.insert(index, *entry);
                }
                _ => order.push(*entry),
            }
        }
        // A group can stand empty, so nothing else names its place.
        for group in &self.groups {
            let entry = SidebarItem::Group(group.id);
            if !order.contains(&entry) {
                order.push(entry);
            }
        }
        // Pinned rows stand above everything else, in their own order.
        let pinned = |entry: &SidebarItem| match entry {
            SidebarItem::Workspace(id) => self
                .workspaces
                .iter()
                .any(|workspace| workspace.id == *id && workspace.pinned),
            _ => false,
        };
        let mut front: Vec<SidebarItem> = order.iter().copied().filter(pinned).collect();
        order.retain(|item| !pinned(item));
        front.extend(order);
        let order = front;
        {
            let order = &order;
            let position = |entry: SidebarItem| order.iter().position(|item| *item == entry);
            self.groups
                .sort_by_key(|group| position(SidebarItem::Group(group.id)));
            self.folders
                .sort_by_key(|folder| position(SidebarItem::Folder(folder.id)));
            self.workspaces.sort_by_key(|workspace| {
                let entry = entries
                    .iter()
                    .find(|(id, _)| *id == workspace.id)
                    .map_or(SidebarItem::Workspace(workspace.id), |(_, entry)| *entry);
                position(entry)
            });
        }
        self.sidebar_order = order;
    }

    pub(crate) fn workspace_mut(&mut self, id: WorkspaceId) -> Result<&mut Workspace, Error> {
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
            .ok_or(Error::UnknownWorkspace(id))
    }
    /// Empties what the pane's agent linked and attached, keeping it under
    /// the conversation it named, if it named one. Reports a change.
    pub(crate) fn set_aside(&mut self, id: PaneId) -> bool {
        let Some(pane) = self
            .workspaces
            .iter_mut()
            .flat_map(|workspace| workspace.panes.iter_mut())
            .find(|pane| pane.id == id)
        else {
            return false;
        };
        if pane.pull_requests.is_empty() && pane.attachments.is_empty() {
            return false;
        }
        let pull_requests = std::mem::take(&mut pane.pull_requests);
        let attachments = std::mem::take(&mut pane.attachments);
        if let Some(crate::AgentSession {
            kind,
            session_id: Some(session_id),
            ..
        }) = &pane.agent
        {
            self.conversations
                .retain(|kept| !(kept.kind == *kind && kept.session_id == *session_id));
            if self.conversations.len() >= crate::Conversation::MAX {
                self.conversations.remove(0);
            }
            self.conversations.push(crate::Conversation {
                kind: *kind,
                session_id: session_id.clone(),
                pull_requests,
                attachments,
            });
        }
        true
    }
    /// Gives the pane what was set aside for the conversation its agent
    /// names, before anything it has linked since. Reports a change.
    pub(crate) fn take_back(&mut self, id: PaneId) -> bool {
        let Some(pane) = self
            .workspaces
            .iter_mut()
            .flat_map(|workspace| workspace.panes.iter_mut())
            .find(|pane| pane.id == id)
        else {
            return false;
        };
        let Some(index) = pane.agent.as_ref().and_then(|agent| {
            self.conversations.iter().position(|kept| {
                kept.kind == agent.kind && Some(&kept.session_id) == agent.session_id.as_ref()
            })
        }) else {
            return false;
        };
        let mut kept = self.conversations.remove(index);
        for link in pane.pull_requests.drain(..) {
            kept.pull_requests.retain(|known| !known.same(&link));
            kept.pull_requests.push(link);
        }
        for file in pane.attachments.drain(..) {
            kept.attachments.retain(|known| known.path() != file.path());
            kept.attachments.push(file);
        }
        let extra = |len: usize, most: usize| ..len.saturating_sub(most);
        kept.pull_requests.drain(extra(
            kept.pull_requests.len(),
            crate::PullRequest::MAX_PER_PANE,
        ));
        kept.attachments.drain(extra(
            kept.attachments.len(),
            crate::Attachment::MAX_PER_PANE,
        ));
        pane.pull_requests = kept.pull_requests;
        pane.attachments = kept.attachments;
        true
    }
    pub(crate) fn pane_mut(&mut self, id: PaneId) -> Result<&mut Pane, Error> {
        self.workspaces
            .iter_mut()
            .flat_map(|workspace| workspace.panes.iter_mut())
            .find(|pane| pane.id == id)
            .ok_or(Error::UnknownPane(id))
    }
}
