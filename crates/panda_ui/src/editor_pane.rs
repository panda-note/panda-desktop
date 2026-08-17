use gpui::{Context, Focusable, StyleRefinement, Window, deferred, div, prelude::*, px};
use markdown::{MarkdownElement, MarkdownFont, MarkdownStyle};
use menu::Confirm;
use search::BufferSearchBar;
use search::buffer_search::DivRegistrar;
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{
    ContextMenu, Divider, DividerColor, DropdownMenu, DropdownStyle, KeyBinding, ListItem,
    ListItemSpacing, TintColor,
};

use crate::panda_actions::{NewMemo, NewTodo};
use crate::shell::AppShell;
use crate::state::WorkspaceMode;
use crate::state::{MemoDisplayMode, Mode};
use crate::widgets::{
    filtered_tag_suggestions, notebook_breadcrumb_path, removable_tag_chip, sync_indicator,
};

impl AppShell {
    pub(crate) fn render_editor_pane(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if matches!(&self.mode, Mode::Main(main) if main.workspace_mode == WorkspaceMode::Todos) {
            return self.render_todo_detail(window, cx);
        }
        let Mode::Main(main) = &self.mode else {
            return div().into_any_element();
        };
        let editor = main.editor.clone();
        let has_memo = editor.is_some();
        let title_editor = main.title_editor.clone();
        let tag_editor = main.tag_editor.clone();
        let buffer_search_bar = main
            .buffer_search_bar
            .clone()
            .filter(|bar| !bar.read(cx).is_dismissed());
        let preview = main.preview.clone();
        let preview_scroll_handle = main.preview_scroll_handle.clone();
        let memo_display_mode = main.memo_display_mode;
        let sync_state = main.active.as_ref().map(|a| a.sync_state);
        let active_id = main.active.as_ref().map(|a| a.id.clone());
        let active_tags = main
            .active
            .as_ref()
            .map(|a| a.tags.clone())
            .unwrap_or_default();
        let style = memo_display_mode
            .shows_preview()
            .then(|| MarkdownStyle::themed(MarkdownFont::Preview, window, cx));

        let title_row = has_memo.then(|| {
            let tag_query = tag_editor
                .as_ref()
                .map(|editor| editor.read(cx).text(cx).trim().to_string())
                .unwrap_or_default();
            let suggested_tags =
                filtered_tag_suggestions(&main.available_tags, &active_tags, &tag_query);
            let show_tag_suggestions = !tag_query.is_empty() && !suggested_tags.is_empty();

            let breadcrumb = main
                .active
                .as_ref()
                .map(|active| notebook_breadcrumb_path(&main.notebooks, &active.notebook_id))
                .unwrap_or_default();
            let breadcrumb_text = breadcrumb.join(" / ");

            let mut header_left = h_flex().flex_1().min_w_0().gap_2().items_center();
            if !breadcrumb_text.is_empty() {
                header_left = header_left
                    .child(
                        Label::new(breadcrumb_text)
                            .size(LabelSize::XSmall)
                            .color(Color::Muted)
                            .truncate(),
                    )
                    .child(Label::new("/").size(LabelSize::XSmall).color(Color::Muted));
            }
            if let Some(te) = title_editor {
                header_left = header_left.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(26.))
                        .flex()
                        .items_center()
                        .on_action(cx.listener(|this, _: &editor::actions::Tab, window, cx| {
                            if let Mode::Main(main) = &this.mode {
                                if let Some(tag) = &main.tag_editor {
                                    window.focus(&tag.focus_handle(cx), cx);
                                }
                            }
                        }))
                        .child(te),
                );
            }

            let mut tag_strip = h_flex().gap_1().items_center().flex_none();
            for tag in active_tags {
                let remove_tag = tag.clone();
                let memo_id = active_id.clone().unwrap_or_default();
                tag_strip = tag_strip.child(removable_tag_chip(
                    tag,
                    move |this, _, _window, cx| {
                        this.remove_tag(memo_id.clone(), remove_tag.clone(), cx);
                    },
                    cx,
                ));
            }
            if let Some(te) = tag_editor.clone() {
                let tag_focus = te.read(cx).focus_handle(cx);
                tag_strip = tag_strip.child(
                    div()
                        .id("tag-input")
                        .h(px(22.))
                        .w(px(110.))
                        .px_1()
                        .flex()
                        .items_center()
                        .rounded_sm()
                        .border_1()
                        .border_color(cx.theme().colors().border_variant)
                        .bg(cx.theme().colors().ghost_element_background)
                        .track_focus(&tag_focus)
                        .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                            if let Mode::Main(main) = &this.mode {
                                if let Some(tag_editor) = &main.tag_editor {
                                    let text = tag_editor.read(cx).text(cx);
                                    this.commit_tag_from_editor(text, window, cx);
                                }
                            }
                        }))
                        .on_action(cx.listener(|this, _: &editor::actions::Tab, window, cx| {
                            if let Mode::Main(main) = &this.mode {
                                if let Some(editor) = &main.editor {
                                    window.focus(&editor.focus_handle(cx), cx);
                                }
                            }
                        }))
                        .on_action(
                            cx.listener(|this, _: &editor::actions::Backtab, window, cx| {
                                if let Mode::Main(main) = &this.mode {
                                    if let Some(title) = &main.title_editor {
                                        window.focus(&title.focus_handle(cx), cx);
                                    }
                                }
                            }),
                        )
                        .child(te),
                );
            }

            let tag_suggestions = show_tag_suggestions.then(|| {
                deferred(
                    div()
                        .absolute()
                        .top_full()
                        .right(px(8.))
                        .mt_1()
                        .w(px(220.))
                        .p_1()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().colors().border)
                        .bg(cx.theme().colors().elevated_surface_background)
                        .shadow_md()
                        .occlude()
                        .children(suggested_tags.into_iter().map(|tag| {
                            let tag_name = tag.clone();
                            ListItem::new(format!("tag-suggest-{tag}"))
                                .spacing(ListItemSpacing::Dense)
                                .inset(true)
                                .child(Label::new(tag).size(LabelSize::Small))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.add_tag_to_active(tag_name.clone(), cx);
                                    this.clear_tag_editor(window, cx);
                                }))
                        })),
                )
                .with_priority(1)
            });

            div()
                .relative()
                .w_full()
                .px_3()
                .py_1p5()
                .border_b_1()
                .border_color(cx.theme().colors().border)
                .child(
                    h_flex()
                        .w_full()
                        .gap_3()
                        .items_center()
                        .child(header_left)
                        .child(tag_strip)
                        .child(
                            h_flex()
                                .gap_2()
                                .flex_none()
                                .children(sync_state.map(sync_indicator)),
                        ),
                )
                .children(tag_suggestions)
        });

        let mut body = h_flex().flex_1().w_full().min_w_0().min_h_0().overflow_hidden();
        if let Some(editor) = editor {
            if memo_display_mode.shows_editor() {
                body = body.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .h_full()
                        .p_2()
                        .child(
                            editor.cached(StyleRefinement::default().size_full()),
                        ),
                );
            }
        } else {
            let focus = self.focus_handle(cx);
            let new_memo_binding = KeyBinding::for_action_in(&NewMemo, &focus, cx);
            body = body.child(
                v_flex()
                    .id("memo-empty-state")
                    .flex_1()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .p_4()
                    .child(
                        v_flex()
                            .w(px(220.))
                            .max_w_full()
                            .gap_1()
                            .child(
                                v_flex()
                                    .mb_2()
                                    .gap_0p5()
                                    .items_center()
                                    .child(
                                        Label::new("Select a memo from the list")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    )
                                    .child(
                                        Label::new("to start editing")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    ),
                            )
                            .child(
                                Button::new("empty-new-memo", "New Memo")
                                    .full_width()
                                    .key_binding(new_memo_binding)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.create_memo(window, cx);
                                    })),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(Divider::horizontal().color(DividerColor::Border))
                                    .child(
                                        Label::new("or")
                                            .size(LabelSize::XSmall)
                                            .color(Color::Muted),
                                    )
                                    .child(Divider::horizontal().color(DividerColor::Border)),
                            )
                            .child(
                                Button::new("empty-refresh", "Refresh")
                                    .full_width()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.refresh_remote(window, cx);
                                    })),
                            ),
                    ),
            );
        }
        if memo_display_mode.shows_preview() {
            if let (Some(preview), Some(style)) = (preview, style) {
                let mut element = MarkdownElement::new(preview, style);
                if memo_display_mode == MemoDisplayMode::Live {
                    element = element
                        .scroll_handle(preview_scroll_handle.clone())
                        .show_root_block_markers();
                }
                body = body.child(
                    div()
                        .id(if memo_display_mode == MemoDisplayMode::Live {
                            "memo-live-preview"
                        } else {
                            "memo-read-preview"
                        })
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .h_full()
                        .p_3()
                        .overflow_y_scroll()
                        .track_scroll(&preview_scroll_handle)
                        .child(element),
                );
            }
        }

        let search_overlay = buffer_search_bar.map(|bar| {
            deferred(
                div()
                    .absolute()
                    .top(px(8.))
                    .right(px(12.))
                    .w(px(420.))
                    .max_w_full()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().elevated_surface_background)
                    .shadow_md()
                    .p_2()
                    .occlude()
                    .child(bar),
            )
            .with_priority(1)
        });

        let pane = v_flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .relative()
            .bg(cx.theme().colors().editor_background)
            .children(title_row)
            .children(self.render_format_toolbar(window, cx))
            .child(body)
            .children(search_overlay);

        let mut registrar = DivRegistrar::new(Self::buffer_search_bar_entity, cx);
        BufferSearchBar::register(&mut registrar);
        registrar
            .into_div()
            .flex_1()
            .h_full()
            .min_w_0()
            .size_full()
            .child(pane)
            .into_any_element()
    }

    fn render_todo_detail(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Mode::Main(main) = &self.mode else {
            return div().into_any_element();
        };
        let todo = main
            .selected_todo_id
            .as_deref()
            .and_then(|id| main.todos.iter().find(|(todo, _)| todo.id == id))
            .map(|(todo, _)| todo.clone());
        let title_editor = main.todo_title_editor.clone();
        let note_editor = main.todo_note_editor.clone();
        let due_date_editor = main.todo_due_date_editor.clone();
        let priority_editor = main.todo_priority_editor.clone();
        let todo_save_in_flight = main.todo_save_in_flight;
        let mut pane = v_flex()
            .key_context("Todos")
            .flex_1()
            .h_full()
            .min_w_0()
            .p_4()
            .gap_2()
            .bg(cx.theme().colors().editor_background)
            .capture_action(cx.listener(|this, _: &NewMemo, window, cx| {
                cx.stop_propagation();
                this.create_todo(window, cx);
            }));
        if let Some(todo) = todo {
            let id = todo.id.clone();
            let done = todo.status == panda_core::TodoStatus::Completed;
            let due_label = due_date_editor
                .as_ref()
                .map(|editor| editor.read(cx).text(cx))
                .filter(|due| !due.is_empty())
                .unwrap_or_else(|| "No due date".to_string());
            let priority = priority_editor
                .as_ref()
                .and_then(|editor| editor.read(cx).text(cx).parse::<i64>().ok())
                .unwrap_or(0)
                .clamp(0, 3);
            let due_menu = {
                let Mode::Main(main) = &mut self.mode else {
                    return div().into_any_element();
                };
                if main.todo_due_menu.is_none() {
                    main.todo_due_menu = Some(todo_due_menu(window, cx));
                }
                main.todo_due_menu.clone().unwrap()
            };
            let priority_menu = {
                let Mode::Main(main) = &mut self.mode else {
                    return div().into_any_element();
                };
                if main.todo_priority_menu.is_none() {
                    main.todo_priority_menu = Some(todo_priority_menu(window, cx));
                }
                main.todo_priority_menu.clone().unwrap()
            };
            let save_label = if todo_save_in_flight {
                "Saving…"
            } else {
                "Save"
            };
            let title = title_editor.map(|editor| {
                let focus = editor.read(cx).focus_handle(cx);
                div()
                    .id("todo-title-input")
                    .key_context("Todos")
                    .h(px(36.))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .px_2()
                    .track_focus(&focus)
                    .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                        this.focus_todo_note(window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &editor::actions::Tab, window, cx| {
                        this.focus_todo_note(window, cx);
                    }))
                    .child(editor)
            });
            let metadata = h_flex()
                .w_full()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(cx.theme().colors().element_background)
                .child(
                    Label::new("Due")
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                )
                .child(
                    DropdownMenu::new("todo-due-date", due_label, due_menu)
                        .style(DropdownStyle::Subtle),
                )
                .child(
                    Label::new("Priority")
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                )
                .child(
                    DropdownMenu::new("todo-priority", priority_label(priority), priority_menu)
                        .style(DropdownStyle::Subtle),
                );
            pane = pane
                .child(
                    h_flex()
                        .gap_2()
                        .w_full()
                        .child(
                            div()
                                .id("todo-detail-check")
                                .w(px(16.))
                                .h(px(16.))
                                .flex_none()
                                .rounded_full()
                                .border_1()
                                .border_color(if done {
                                    cx.theme().status().success_border
                                } else {
                                    cx.theme().colors().text_muted
                                })
                                .when(done, |circle| {
                                    circle.bg(cx.theme().status().success_background)
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                                .when(done, |circle| {
                                    circle.child(
                                        Icon::new(IconName::Check)
                                            .size(IconSize::XSmall)
                                            .color(Color::Success),
                                    )
                                })
                                .tooltip(ui::Tooltip::text(if done {
                                    "Restore task"
                                } else {
                                    "Complete task"
                                }))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.toggle_todo(id.clone(), window, cx);
                                })),
                        )
                        .children(title)
                        .child(
                            Button::new("todo-save", save_label)
                                .style(ButtonStyle::Tinted(TintColor::Accent))
                                .disabled(todo_save_in_flight)
                                .tooltip(ui::Tooltip::text("Save task (Ctrl+S)"))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_todo(window, cx)),
                                ),
                        ),
                )
                .child(metadata)
                .child(Divider::horizontal().color(DividerColor::BorderFaded))
                .children(note_editor.map(|editor| {
                    div()
                        .key_context("Todos")
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().colors().editor_background)
                        .capture_action(cx.listener(
                            |this, _: &editor::actions::Backtab, window, cx| {
                                cx.stop_propagation();
                                this.focus_todo_title(window, cx);
                            },
                        ))
                        .child(editor)
                }));
        } else {
            let focus = self.focus_handle(cx);
            let new_todo_binding = KeyBinding::for_action_in(&NewTodo, &focus, cx);
            pane = pane.child(
                v_flex()
                    .flex_1()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        Label::new("Select a task")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        Button::new("empty-new-todo", "New Task")
                            .key_binding(new_todo_binding)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_todo(window, cx);
                            })),
                    ),
            );
        }
        div()
            .flex_1()
            .h_full()
            .min_w_0()
            .min_h_0()
            .size_full()
            .child(pane)
            .into_any_element()
    }

    fn buffer_search_bar_entity(
        this: &Self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<gpui::Entity<BufferSearchBar>> {
        match &this.mode {
            Mode::Main(main) => main.buffer_search_bar.clone(),
            Mode::Setup(_) => None,
        }
    }
}

fn todo_due_menu(window: &mut Window, cx: &mut Context<AppShell>) -> gpui::Entity<ContextMenu> {
    let entity = cx.weak_entity();
    let today = chrono::Local::now().date_naive();
    let tomorrow = (today + chrono::Days::new(1)).to_string();
    let next_week = (today + chrono::Days::new(7)).to_string();
    let today = today.to_string();
    ContextMenu::build(window, cx, move |menu, _, _| {
        let no_date = entity.clone();
        let today_entity = entity.clone();
        let tomorrow_entity = entity.clone();
        let next_week_entity = entity.clone();
        menu.entry("No due date", None, move |window, cx| {
            no_date
                .update(cx, |this, cx| this.set_todo_due_date(None, window, cx))
                .ok();
        })
        .entry("Today", None, move |window, cx| {
            today_entity
                .update(cx, |this, cx| {
                    this.set_todo_due_date(Some(today.clone()), window, cx)
                })
                .ok();
        })
        .entry("Tomorrow", None, move |window, cx| {
            tomorrow_entity
                .update(cx, |this, cx| {
                    this.set_todo_due_date(Some(tomorrow.clone()), window, cx)
                })
                .ok();
        })
        .entry("In one week", None, move |window, cx| {
            next_week_entity
                .update(cx, |this, cx| {
                    this.set_todo_due_date(Some(next_week.clone()), window, cx)
                })
                .ok();
        })
    })
}

fn todo_priority_menu(
    window: &mut Window,
    cx: &mut Context<AppShell>,
) -> gpui::Entity<ContextMenu> {
    let entity = cx.weak_entity();
    ContextMenu::build(window, cx, move |menu, _, _| {
        let none = entity.clone();
        let low = entity.clone();
        let medium = entity.clone();
        let high = entity.clone();
        menu.entry("No priority", None, move |window, cx| {
            none.update(cx, |this, cx| this.set_todo_priority(0, window, cx))
                .ok();
        })
        .entry("Low", None, move |window, cx| {
            low.update(cx, |this, cx| this.set_todo_priority(1, window, cx))
                .ok();
        })
        .entry("Medium", None, move |window, cx| {
            medium
                .update(cx, |this, cx| this.set_todo_priority(2, window, cx))
                .ok();
        })
        .entry("High", None, move |window, cx| {
            high.update(cx, |this, cx| this.set_todo_priority(3, window, cx))
                .ok();
        })
    })
}

fn priority_label(priority: i64) -> &'static str {
    match priority {
        1 => "Low",
        2 => "Medium",
        3 => "High",
        _ => "No priority",
    }
}
