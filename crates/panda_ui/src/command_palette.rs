//! Lightweight command palette for Panda actions.

use editor::actions::{
    Format as EditorFormat, InsertHorizontalRule, InsertLink, Redo, SelectAll, ToggleBlockQuote,
    ToggleBold, ToggleCodeBlock, ToggleHeading1, ToggleHeading2, ToggleHeading3, ToggleHeading4,
    ToggleHeading5, ToggleHeading6, ToggleInlineCode, ToggleItalic, ToggleLineNumbers,
    ToggleOrderedList, ToggleStrikethrough, ToggleTaskList, ToggleUnorderedList, Undo,
};
use editor::{Bias, Editor, EditorEvent, scroll::Autoscroll};
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, KeyContext, Render,
    SharedString, Subscription, Window, deferred, div, prelude::*, px,
};
use language::Point;
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{Button, KeyBinding, ListItem, ListItemSpacing};

use crate::panda_actions::{
    DeleteMemo, FormatMemo, GoToLine, NewMemo, OpenInstances, OpenSettings, Refresh, SaveMemo,
    ToggleNavPane, TogglePreview, ToggleStatusBar,
};
use crate::shell::AppShell;
use crate::state::Mode;

enum PaletteItem {
    Action {
        label: SharedString,
        action: Box<dyn gpui::Action>,
    },
    GoToLine {
        line: u32,
        label: SharedString,
    },
}

pub struct PandaCommandPalette {
    query: Entity<Editor>,
    entries: Vec<(SharedString, Box<dyn gpui::Action>)>,
    filtered: Vec<PaletteItem>,
    selected: usize,
    previous_focus: Option<FocusHandle>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

fn palette_query_editor(window: &mut Window, cx: &mut Context<Editor>) -> Editor {
    let mut editor = Editor::single_line(window, cx);
    editor.set_placeholder_text("Type a command or line number…", window, cx);
    editor.set_show_gutter(false, cx);
    editor.set_show_line_numbers(false, cx);
    editor.set_show_breakpoints(false, cx);
    editor.set_use_modal_editing(false);
    editor
}

impl PandaCommandPalette {
    pub fn new(
        previous_focus: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| palette_query_editor(window, cx));
        let focus_handle = cx.focus_handle();
        let entries = panda_entries();

        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe(&query, |this, _, event: &EditorEvent, cx| {
            if matches!(
                event,
                EditorEvent::BufferEdited | EditorEvent::Edited { .. }
            ) {
                this.refilter(cx);
            }
        }));

        let mut this = Self {
            query: query.clone(),
            entries,
            filtered: Vec::new(),
            selected: 0,
            previous_focus,
            focus_handle,
            _subscriptions: subscriptions,
        };
        this.refilter(cx);
        this
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        let raw = self.query.read(cx).text(cx);
        let q = raw.trim().to_lowercase();
        let mut items = Vec::new();

        if let Some(line) = parse_go_to_line_query(&raw) {
            items.push(PaletteItem::GoToLine {
                line,
                label: format!("Navigate: Go to Line {line}").into(),
            });
        }

        for (label, action) in &self.entries {
            if q.is_empty()
                || label.to_lowercase().contains(&q)
                || action.name().to_lowercase().contains(&q)
            {
                items.push(PaletteItem::Action {
                    label: label.clone(),
                    action: action.boxed_clone(),
                });
            }
        }

        self.filtered = items;
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
        cx.notify();
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.filtered.get(self.selected) else {
            return;
        };
        match item {
            PaletteItem::Action { action, .. } => {
                let action = action.boxed_clone();
                if let Some(prev) = self.previous_focus.take() {
                    window.focus(&prev, cx);
                }
                cx.emit(DismissEvent);
                window.defer(cx, move |window, cx| {
                    window.dispatch_action(action, cx);
                });
            }
            PaletteItem::GoToLine { line, .. } => {
                let line = *line;
                if let Some(prev) = self.previous_focus.take() {
                    window.focus(&prev, cx);
                }
                cx.emit(DismissEvent);
                window.defer(cx, move |window, cx| {
                    jump_to_line_in_active_editor(line, window, cx);
                });
            }
        }
    }
}

fn parse_go_to_line_query(raw: &str) -> Option<u32> {
    let trimmed = raw.trim().trim_start_matches(':').trim();
    if trimmed.is_empty() {
        return None;
    }
    // Plain number, or `12:3` / `12,3` style — use the line part only.
    let line_part = trimmed
        .split([':', ',', ' '])
        .next()
        .unwrap_or(trimmed)
        .trim();
    if !line_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let line: u32 = line_part.parse().ok()?;
    (line >= 1).then_some(line)
}

fn jump_to_line_in_active_editor(line: u32, _window: &mut Window, cx: &mut App) {
    let Some(handle) = cx
        .windows()
        .into_iter()
        .find_map(|w| w.downcast::<AppShell>())
    else {
        return;
    };
    handle
        .update(cx, |shell, window, cx| {
            let Mode::Main(main) = &shell.mode else {
                return;
            };
            let Some(editor) = main.editor.clone() else {
                return;
            };
            editor.update(cx, |editor, cx| {
                let snapshot = editor.buffer().read(cx).snapshot(cx);
                let max_row = snapshot.max_point().row;
                let row = line.saturating_sub(1).min(max_row);
                let target = snapshot.clip_point(Point::new(row, 0), Bias::Left);
                editor.change_selections(Default::default(), window, cx, |s| {
                    s.select_ranges([target..target]);
                });
                editor.request_autoscroll(Autoscroll::center(), cx);
            });
        })
        .ok();
}

impl EventEmitter<DismissEvent> for PandaCommandPalette {}

impl Focusable for PandaCommandPalette {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PandaCommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let previous_focus = self.previous_focus.clone();
        let list = self
            .filtered
            .iter()
            .enumerate()
            .map(|(row, item)| {
                let (label, action_for_binding) = match item {
                    PaletteItem::Action { label, action } => {
                        (label.clone(), Some(action.boxed_clone()))
                    }
                    PaletteItem::GoToLine { label, .. } => (label.clone(), None),
                };
                let selected = row == self.selected;
                let row_id = SharedString::from(format!("cmd-{row}"));
                ListItem::new(row_id)
                    .inset(true)
                    .spacing(ListItemSpacing::Sparse)
                    .toggle_state(selected)
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .justify_between()
                            .child(Label::new(label).size(LabelSize::Small).truncate())
                            .children(
                                selected
                                    .then(|| previous_focus.as_ref())
                                    .flatten()
                                    .zip(action_for_binding.as_ref())
                                    .map(|(focus, action)| {
                                        KeyBinding::for_action_in(&**action, focus, cx)
                                    }),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.selected = row;
                        this.confirm(window, cx);
                    }))
            })
            .collect::<Vec<_>>();

        let mut key_context = KeyContext::default();
        key_context.add("CommandPalette");
        key_context.add("Picker");

        let focus_for_footer = previous_focus.clone();
        deferred(
            v_flex()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::hsla(0., 0., 0., 0.35))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|_, _, _, cx| {
                        cx.emit(DismissEvent);
                    }),
                )
                .child(
                    v_flex()
                        .w(px(560.))
                        .max_h(px(460.))
                        .occlude()
                        .track_focus(&self.focus_handle)
                        .key_context(key_context)
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().colors().border)
                        .bg(cx.theme().colors().elevated_surface_background)
                        .on_action(cx.listener(|this, _: &menu::Confirm, window, cx| {
                            this.confirm(window, cx);
                        }))
                        .on_action(cx.listener(|_this, _: &menu::Cancel, _, cx| {
                            cx.emit(DismissEvent);
                        }))
                        .on_action(cx.listener(|this, _: &menu::SelectNext, _, cx| {
                            if !this.filtered.is_empty() {
                                this.selected = (this.selected + 1) % this.filtered.len();
                                cx.notify();
                            }
                        }))
                        .on_action(cx.listener(|this, _: &menu::SelectPrevious, _, cx| {
                            if !this.filtered.is_empty() {
                                this.selected = if this.selected == 0 {
                                    this.filtered.len() - 1
                                } else {
                                    this.selected - 1
                                };
                                cx.notify();
                            }
                        }))
                        .child(
                            h_flex()
                                .h(px(36.))
                                .w_full()
                                .px_2p5()
                                .flex_none()
                                .overflow_hidden()
                                .child(div().flex_1().min_w_0().child(self.query.clone())),
                        )
                        .child(
                            v_flex()
                                .id("cmd-list")
                                .flex_1()
                                .max_h(px(360.))
                                .overflow_y_scroll()
                                .gap_0p5()
                                .p_1()
                                .children(list),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .p_1p5()
                                .gap_1()
                                .justify_end()
                                .border_t_1()
                                .border_color(cx.theme().colors().border_variant)
                                .child(
                                    Button::new("run-action", "Run")
                                        .key_binding(focus_for_footer.as_ref().map(|focus| {
                                            KeyBinding::for_action_in(&menu::Confirm, focus, cx)
                                                .size(rems_from_px(12.))
                                        }))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.confirm(window, cx);
                                        })),
                                ),
                        ),
                ),
        )
        .with_priority(2)
    }
}

fn panda_entries() -> Vec<(SharedString, Box<dyn gpui::Action>)> {
    let mut entries = vec![
        // Memo
        entry("Memo: New", NewMemo),
        entry("Memo: Save", SaveMemo),
        entry("Memo: Delete", DeleteMemo),
        entry("Memo: Refresh", Refresh),
        // Navigate
        entry("Navigate: Go to Line…", GoToLine),
        // Editor
        entry("Editor: Format Document", FormatMemo),
        entry("Editor: Format", EditorFormat),
        entry("Editor: Undo", Undo),
        entry("Editor: Redo", Redo),
        entry("Editor: Select All", SelectAll),
        entry("Editor: Toggle Line Numbers", ToggleLineNumbers),
        // Markdown
        entry("Markdown: Heading 1", ToggleHeading1),
        entry("Markdown: Heading 2", ToggleHeading2),
        entry("Markdown: Heading 3", ToggleHeading3),
        entry("Markdown: Heading 4", ToggleHeading4),
        entry("Markdown: Heading 5", ToggleHeading5),
        entry("Markdown: Heading 6", ToggleHeading6),
        entry("Markdown: Bold", ToggleBold),
        entry("Markdown: Italic", ToggleItalic),
        entry("Markdown: Strikethrough", ToggleStrikethrough),
        entry("Markdown: Inline Code", ToggleInlineCode),
        entry("Markdown: Code Block", ToggleCodeBlock),
        entry("Markdown: Block Quote", ToggleBlockQuote),
        entry("Markdown: Unordered List", ToggleUnorderedList),
        entry("Markdown: Ordered List", ToggleOrderedList),
        entry("Markdown: Task List", ToggleTaskList),
        entry("Markdown: Insert Link", InsertLink),
        entry("Markdown: Insert Horizontal Rule", InsertHorizontalRule),
        // View
        entry("View: Toggle Navigation", ToggleNavPane),
        entry("View: Toggle Markdown Preview", TogglePreview),
        entry("View: Toggle Status Bar", ToggleStatusBar),
        // App
        entry("App: Instances…", OpenInstances),
        entry("App: Settings…", OpenSettings),
    ];
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

fn entry(label: &str, action: impl gpui::Action) -> (SharedString, Box<dyn gpui::Action>) {
    (label.into(), Box::new(action))
}

impl AppShell {
    pub(crate) fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_palette.take().is_some() {
            self._palette_sub = None;
            cx.notify();
            return;
        }
        let previous_focus = window.focused(cx);
        let palette = cx.new(|cx| PandaCommandPalette::new(previous_focus, window, cx));
        self._palette_sub = Some(cx.subscribe(&palette, |this, _, _: &DismissEvent, cx| {
            this.command_palette = None;
            this._palette_sub = None;
            cx.notify();
        }));
        window.focus(&palette.focus_handle(cx), cx);
        let query_focus = palette.read(cx).query.focus_handle(cx);
        window.focus(&query_focus, cx);
        self.command_palette = Some(palette);
        cx.notify();
    }

    pub(crate) fn open_go_to_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &self.mode else {
            return;
        };
        let Some(editor) = main.editor.clone() else {
            if let Mode::Main(main) = &mut self.mode {
                main.status = "Open a memo first".into();
            }
            cx.notify();
            return;
        };
        let Some(buffer) = editor.read(cx).active_buffer(cx) else {
            return;
        };
        // Already open — just refocus instead of tearing down and recreating
        // (duplicate app + window action handlers used to do that).
        if let Some(modal) = self.go_to_line.clone() {
            cx.on_next_frame(window, move |_, window, cx| {
                window.focus(&modal.focus_handle(cx), cx);
            });
            return;
        }
        let modal = cx.new(|cx| go_to_line::GoToLine::new(editor, buffer, window, cx));
        self._go_to_line_sub = Some(cx.subscribe(&modal, |this, _, _: &DismissEvent, cx| {
            this.go_to_line = None;
            this._go_to_line_sub = None;
            cx.notify();
        }));
        // Mount first, then focus on the next painted frame so the line editor is
        // in the focus tree (window.defer alone can still race before paint).
        self.go_to_line = Some(modal.clone());
        cx.notify();
        cx.on_next_frame(window, move |_, window, cx| {
            window.focus(&modal.focus_handle(cx), cx);
        });
    }
}
