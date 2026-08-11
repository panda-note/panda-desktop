use gpui::{
    AnyElement, App, Context, Entity, MouseButton, SharedString, deferred, div, prelude::*, px,
};
use platform_title_bar::PlatformTitleBar;
use theme::ActiveTheme;
use ui::Tooltip;
use ui::prelude::*;

use crate::memo_card::{MemoCardData, memo_list_item};
use crate::shell::AppShell;
use crate::state::{Mode, WorkspaceMode};
use crate::widgets::notebook_breadcrumb_path;

impl AppShell {
    pub(crate) fn create_platform_title_bar(cx: &mut Context<Self>) -> Entity<PlatformTitleBar> {
        cx.new(|cx| PlatformTitleBar::new("panda-title-bar", cx))
    }

    pub(crate) fn sync_title_bar(&mut self, cx: &mut Context<Self>) {
        let Some(title_bar) = self.title_bar.clone() else {
            return;
        };
        let children = self.title_bar_children(cx);
        // PlatformTitleBar::render mem::take's its children, so this must run in
        // AppShell::render immediately before the title bar is painted (Zed's
        // TitleBar uses the same pattern). Do not notify here — we are already
        // inside a render pass that will paint the title bar as a child.
        title_bar.update(cx, |bar, _cx| {
            bar.set_children(children);
        });
    }

    fn title_bar_children(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let show_actions = matches!(&self.mode, Mode::Main(_));
        let todo_workspace =
            matches!(&self.mode, Mode::Main(main) if main.workspace_mode == WorkspaceMode::Todos);
        let todo_trash = matches!(
            &self.mode,
            Mode::Main(main)
                if main.workspace_mode == WorkspaceMode::Todos
                    && main.todo_filter == panda_core::TodoFilter::Trash
        );
        let memo_trash = matches!(
            &self.mode,
            Mode::Main(main)
                if main.workspace_mode == WorkspaceMode::Memos
                    && matches!(main.filter, panda_core::NavFilter::Trash)
        );
        let (search_editor, search_dropdown) = match &self.mode {
            Mode::Main(main) if main.workspace_mode == WorkspaceMode::Memos => (
                main.search_editor.clone(),
                self.render_memo_search_dropdown(cx),
            ),
            _ => (None, None),
        };

        let mut row = h_flex()
            .id("panda-title-content")
            .w_full()
            .h_full()
            .pl_1()
            .pr_1()
            .gap_2()
            .justify_between()
            .child(
                h_flex()
                    .gap_2()
                    .child(self.menu_bar.clone())
                    .child(Label::new("Panda Note").size(LabelSize::Small)),
            );

        row = row.child(
            h_flex()
                .gap_2()
                .items_center()
                .children(search_editor.map(|search| {
                    div()
                        .relative()
                        .w(px(320.))
                        .child(
                            div()
                                .w_full()
                                .h(px(22.))
                                .px_2()
                                .border_1()
                                .rounded_md()
                                .border_color(cx.theme().colors().border)
                                .child(search),
                        )
                        .children(search_dropdown)
                }))
                .child(
                    h_flex()
                        .gap_1()
                        .occlude()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .when(show_actions, |this| {
                            this.child(
                                IconButton::new("title-refresh", IconName::ArrowCircle)
                                    .icon_size(IconSize::Small)
                                    .style(ButtonStyle::Subtle)
                                    .tooltip(Tooltip::text("Refresh"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.refresh_remote(window, cx)
                                    })),
                            )
                            .child(
                                IconButton::new("title-new", IconName::Plus)
                                    .icon_size(IconSize::Small)
                                    .style(ButtonStyle::Subtle)
                                    .tooltip(Tooltip::text(if todo_workspace { "New task" } else { "New memo" }))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if matches!(&this.mode, Mode::Main(main) if main.workspace_mode == WorkspaceMode::Todos) {
                                            this.create_todo(window, cx);
                                        } else {
                                            this.create_memo(window, cx);
                                        }
                                    })),
                            )
                            .when(!todo_workspace && !memo_trash, |this| this.child(
                                IconButton::new("title-delete", IconName::Trash)
                                    .icon_size(IconSize::Small)
                                    .style(ButtonStyle::Subtle)
                                    .tooltip(Tooltip::text("Move memo to trash"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.delete_selected(window, cx)
                                    })),
                            )
                            .when(!todo_workspace && memo_trash, |this| this
                                .child(
                                    IconButton::new("title-memo-restore", IconName::GenericRestore)
                                        .icon_size(IconSize::Small)
                                        .style(ButtonStyle::Subtle)
                                        .tooltip(Tooltip::text("Restore memo"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.restore_selected(window, cx);
                                        })),
                                )
                                .child(
                                    IconButton::new("title-memo-permanent-delete", IconName::Trash)
                                        .icon_size(IconSize::Small)
                                        .style(ButtonStyle::Subtle)
                                        .tooltip(Tooltip::text("Delete permanently (cannot be undone)"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.permanently_delete_selected(window, cx);
                                        })),
                                ),
                            )
                            .when(todo_workspace && !todo_trash, |this| this.child(
                                IconButton::new(
                                    "title-todo-delete",
                                    IconName::Trash,
                                )
                                    .icon_size(IconSize::Small)
                                    .style(ButtonStyle::Subtle)
                                    .tooltip(Tooltip::text("Move task to trash"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.delete_selected_todo(window, cx);
                                    })),
                            ))
                            .when(todo_workspace && todo_trash, |this| this
                                .child(
                                    IconButton::new("title-todo-restore", IconName::GenericRestore)
                                        .icon_size(IconSize::Small)
                                        .style(ButtonStyle::Subtle)
                                        .tooltip(Tooltip::text("Restore task"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.restore_selected_todo(window, cx);
                                        })),
                                )
                                .child(
                                    IconButton::new("title-todo-permanent-delete", IconName::Trash)
                                        .icon_size(IconSize::Small)
                                        .style(ButtonStyle::Subtle)
                                        .tooltip(Tooltip::text("Delete permanently (cannot be undone)"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.permanently_delete_selected_todo(window, cx);
                                        })),
                                ),
                            ))
                            .when(todo_workspace && !todo_trash, |this| this.child(
                                IconButton::new("title-todo-save", IconName::CheckDouble)
                                    .icon_size(IconSize::Small)
                                    .style(ButtonStyle::Subtle)
                                    .tooltip(Tooltip::text("Save task"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_todo(window, cx)
                                    })),
                            ))
                            .when(todo_workspace && !todo_trash, |this| this.child(
                                IconButton::new("title-todo-complete", IconName::Check)
                                    .icon_size(IconSize::Small)
                                    .style(ButtonStyle::Subtle)
                                    .tooltip(Tooltip::text("Complete or restore task"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_selected_todo(window, cx)
                                    })),
                            ))
                            .when(!todo_workspace, |this| this.child(
                                IconButton::new("title-preview", IconName::Eye)
                                    .icon_size(IconSize::Small)
                                    .style(ButtonStyle::Subtle)
                                    .tooltip(Tooltip::text("Cycle Source / Live / Read"))
                                    .on_click(
                                        cx.listener(|this, _, _window, cx| this.toggle_preview(cx)),
                            )),
                            )
                        })
                        .child(
                            IconButton::new("title-instances", IconName::Server)
                                .icon_size(IconSize::Small)
                                .style(ButtonStyle::Subtle)
                                .tooltip(Tooltip::text("Instances"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_instances_window(window, cx)
                                })),
                        )
                        .child(
                            IconButton::new("title-settings", IconName::Settings)
                                .icon_size(IconSize::Small)
                                .style(ButtonStyle::Subtle)
                                .tooltip(Tooltip::text("Settings"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_settings_window(window, cx)
                                })),
                        ),
                ),
        );

        vec![row.into_any_element()]
    }

    fn render_memo_search_dropdown(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Mode::Main(main) = &self.mode else {
            return None;
        };
        let query = main.search_query.trim();
        if query.is_empty() {
            return None;
        }
        let results = main.search_results.clone().unwrap_or_default();
        let notebooks = main.notebooks.clone();
        let selected = main.selected_id.clone();
        let mut list = v_flex()
            .gap_0p5()
            .max_h(px(400.))
            .p_1()
            .bg(cx.theme().colors().panel_background);
        if results.is_empty() {
            list = list.child(
                div().px_3().py_4().child(
                    Label::new("No results")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                ),
            );
        }
        for (memo, sync) in results {
            let memo_id = memo.id.clone();
            let notebook_name = {
                let path = notebook_breadcrumb_path(&notebooks, &memo.notebook_id);
                if path.is_empty() {
                    "Unknown".to_string()
                } else {
                    path.join(" / ")
                }
            };
            let is_sel = selected.as_deref() == Some(memo_id.as_str());
            let card = MemoCardData {
                title: memo.display_title().to_string(),
                excerpt: memo.excerpt.clone(),
                tags: memo.tags.clone(),
                notebook_name: Some(notebook_name),
                is_pinned: memo.is_pinned,
                updated_at: None,
                highlight_query: Some(query.to_string()),
            };
            list = list.child(
                div().w_full().child(
                    memo_list_item(
                        SharedString::from(format!("search-{memo_id}")),
                        card,
                        sync,
                        is_sel,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_search_result(&memo_id, window, cx);
                    })),
                ),
            );
        }
        Some(
            deferred(
                div()
                    .absolute()
                    .top_full()
                    .left_0()
                    .mt_1()
                    .w_full()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().panel_background)
                    .shadow_md()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(list),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    pub(crate) fn render_error_banner(&self, cx: &App) -> Option<AnyElement> {
        let error = match &self.mode {
            Mode::Setup(setup) => setup.error.clone(),
            Mode::Main(main) => main.error.clone(),
        }?;
        Some(
            gpui::div()
                .px_3()
                .py_1()
                .bg(cx.theme().status().error_background)
                .child(Label::new(error).size(LabelSize::Small).color(Color::Error))
                .into_any_element(),
        )
    }
}
