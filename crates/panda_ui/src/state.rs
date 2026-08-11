use std::collections::HashSet;
use std::sync::Arc;

use editor::Editor;
use gpui::{
    Empty, Entity, Pixels, Point, Render, ScrollHandle, SharedString, Subscription, px,
};
use panda_api::ApiClient;
use panda_core::{
    ActiveMemo, Instance, MemoSummary, NavFilter, Notebook, SyncState, Todo, TodoFilter,
};
use panda_store::LocalStore;
use ui::ContextMenu;

pub(crate) const DEFAULT_NAV_WIDTH: f32 = 180.;
pub(crate) const DEFAULT_LIST_WIDTH: f32 = 280.;
pub(crate) const MIN_PANE_WIDTH: f32 = 120.;
pub(crate) const MAX_PANE_WIDTH: f32 = 480.;

pub(crate) enum Mode {
    Setup(SetupState),
    Main(MainState),
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum WorkspaceMode {
    #[default]
    Memos,
    Todos,
}

/// Where a dragged notebook will be inserted relative to a hover target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotebookInsertSide {
    Before,
    After,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotebookDropIndicator {
    pub target_id: String,
    pub side: NotebookInsertSide,
}

/// How memo Markdown is presented.  The document always remains the same
/// `editor::Editor` buffer; this only changes its projection on screen.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum MemoDisplayMode {
    /// The established editor-only experience (including Vim and buffer search).
    #[default]
    Source,
    /// Side-by-side editor + rendered preview. Preview scroll follows the
    /// editor only when the editor itself scrolls — plain Vim j/k inside the
    /// viewport does not notify or refresh the shell.
    Live,
    /// A distraction-free rendered view. Switching back never changes the buffer.
    Read,
}

impl MemoDisplayMode {
    pub(crate) fn shows_editor(self) -> bool {
        !matches!(self, Self::Read)
    }

    pub(crate) fn shows_preview(self) -> bool {
        matches!(self, Self::Live | Self::Read)
    }

    pub(crate) fn cycle(self) -> Self {
        match self {
            Self::Source => Self::Live,
            Self::Live => Self::Read,
            Self::Read => Self::Source,
        }
    }
}

pub(crate) struct SetupState {
    pub name: Entity<Editor>,
    pub api_url: Entity<Editor>,
    pub username: Entity<Editor>,
    pub password: Entity<Editor>,
    pub token: Entity<Editor>,
    pub error: Option<SharedString>,
    pub busy: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizePane {
    Nav,
    List,
}

#[derive(Clone)]
pub(crate) struct DraggedPane(pub ResizePane);

impl Render for DraggedPane {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        Empty
    }
}

#[derive(Clone)]
pub(crate) struct DraggedMemo {
    pub id: String,
    pub title: SharedString,
    pub anchor: Option<Point<Pixels>>,
}

#[derive(Clone)]
pub(crate) struct DraggedNotebook {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: SharedString,
    pub anchor: Option<Point<Pixels>>,
}

impl Render for DraggedNotebook {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::prelude::*;
        use theme::ActiveTheme;
        use ui::prelude::*;
        let card = h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(cx.theme().colors().elevated_surface_background)
            .border_1()
            .border_color(cx.theme().colors().border)
            .child(
                Icon::new(IconName::Book)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(Label::new(self.name.clone()).size(LabelSize::Small));
        if let Some(anchor) = self.anchor {
            div()
                .pl(anchor.x - px(40.))
                .pt(anchor.y - px(12.))
                .child(card)
        } else {
            card
        }
    }
}

impl Render for DraggedMemo {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::prelude::*;
        use theme::ActiveTheme;
        use ui::prelude::*;
        let card = div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(cx.theme().colors().elevated_surface_background)
            .border_1()
            .border_color(cx.theme().colors().border)
            .child(Label::new(self.title.clone()).size(LabelSize::Small));
        if let Some(anchor) = self.anchor {
            div()
                .pl(anchor.x - px(60.))
                .pt(anchor.y - px(12.))
                .child(card)
        } else {
            card
        }
    }
}

pub(crate) enum RenameTarget {
    Memo(String),
    Notebook(String),
}

pub(crate) struct RenameDialog {
    pub target: RenameTarget,
    pub editor: Entity<Editor>,
}

pub(crate) struct MainState {
    pub instance: Instance,
    pub client: ApiClient,
    pub store: Arc<LocalStore>,
    pub notebooks: Vec<Notebook>,
    pub memos: Vec<(MemoSummary, SyncState)>,
    pub workspace_mode: WorkspaceMode,
    pub todo_filter: TodoFilter,
    pub todos: Vec<(Todo, SyncState)>,
    pub selected_todo_id: Option<String>,
    pub todo_title_editor: Option<Entity<Editor>>,
    pub todo_note_editor: Option<Entity<Editor>>,
    pub todo_due_date_editor: Option<Entity<Editor>>,
    pub todo_priority_editor: Option<Entity<Editor>>,
    pub todo_create_in_flight: bool,
    pub todo_save_in_flight: bool,
    pub filter: NavFilter,
    pub selected_id: Option<String>,
    pub active: Option<ActiveMemo>,
    pub editor: Option<Entity<Editor>>,
    pub title_editor: Option<Entity<Editor>>,
    pub tag_editor: Option<Entity<Editor>>,
    pub search_editor: Option<Entity<Editor>>,
    pub buffer_search_bar: Option<Entity<search::BufferSearchBar>>,
    pub search_query: String,
    pub search_results: Option<Vec<(MemoSummary, SyncState)>>,
    pub available_tags: Vec<String>,
    pub preview: Option<Entity<markdown::Markdown>>,
    pub preview_scroll_handle: ScrollHandle,
    pub memo_display_mode: MemoDisplayMode,
    /// Cached format-toolbar heading menu. Rebuilt only when missing — never on
    /// every AppShell paint (Vim motion dirties the shell via ancestor walks).
    pub heading_menu: Option<Entity<ContextMenu>>,
    pub todo_due_menu: Option<Entity<ContextMenu>>,
    pub todo_priority_menu: Option<Entity<ContextMenu>>,
    pub side_chrome: Option<Entity<crate::chrome::SideChrome>>,
    pub editor_chrome: Option<Entity<crate::chrome::EditorChrome>>,
    pub status: SharedString,
    pub error: Option<SharedString>,
    pub save_generation: u64,
    /// Generation counters coalesce expensive post-edit work.  The editor itself
    /// remains synchronous; disk and rendered projections only see settled text.
    pub draft_generation: u64,
    pub preview_generation: u64,
    pub save_in_flight: bool,
    pub save_pending: bool,
    pub nav_width: Pixels,
    pub list_width: Pixels,
    pub nav_collapsed: bool,
    pub collapsed_notebooks: HashSet<String>,
    /// Insert-line affordance while reordering notebooks (before/after a row).
    pub notebook_drop_indicator: Option<NotebookDropIndicator>,
    pub context_menu: Option<(Entity<ContextMenu>, Point<Pixels>, Subscription)>,
    pub rename_dialog: Option<RenameDialog>,
    pub _rename_sub: Option<Subscription>,
    pub _title_sub: Option<Subscription>,
    pub _tag_sub: Option<Subscription>,
    pub _search_sub: Option<Subscription>,
    pub _buffer_search_sub: Option<Subscription>,
    pub _buffer_sub: Option<Subscription>,
    /// Live preview scroll sync — listens for editor scroll, not selection motion.
    pub _preview_scroll_sub: Option<Subscription>,
}

impl MainState {
    pub fn load_pane_widths(store: &LocalStore) -> (Pixels, Pixels) {
        let nav = store
            .get_meta("nav_width")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(DEFAULT_NAV_WIDTH);
        let list = store
            .get_meta("list_width")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(DEFAULT_LIST_WIDTH);
        (
            px(nav.clamp(MIN_PANE_WIDTH, MAX_PANE_WIDTH)),
            px(list.clamp(MIN_PANE_WIDTH, MAX_PANE_WIDTH)),
        )
    }

    pub fn persist_pane_widths(&self) {
        let _ = self
            .store
            .set_meta("nav_width", &format!("{}", f32::from(self.nav_width)));
        let _ = self
            .store
            .set_meta("list_width", &format!("{}", f32::from(self.list_width)));
    }

    pub fn load_nav_collapsed(store: &LocalStore) -> bool {
        store
            .get_meta("nav_collapsed")
            .ok()
            .flatten()
            .is_some_and(|s| s == "1" || s.eq_ignore_ascii_case("true"))
    }

    pub fn persist_nav_collapsed(&self) {
        let _ = self
            .store
            .set_meta("nav_collapsed", if self.nav_collapsed { "1" } else { "0" });
    }

    pub fn inbox_id(&self) -> Option<String> {
        self.notebooks
            .iter()
            .find(|n| n.slug.as_deref() == Some("inbox"))
            .map(|n| n.id.clone())
            .or_else(|| self.notebooks.first().map(|n| n.id.clone()))
    }
}
