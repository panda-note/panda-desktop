use gpui::{AnyElement, Context, Focusable, SharedString, Window, div, prelude::*, px};
use panda_core::{NavFilter, TodoFilter};
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{ContextMenu, KeyBinding};

use crate::memo_card::{MemoCardData, memo_list_item};
use crate::panda_actions::{NewMemo, NewTodo};
use crate::shell::AppShell;
use crate::state::{DraggedMemo, Mode, WorkspaceMode};

impl AppShell {
    pub(crate) fn render_memo_list(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Mode::Main(main) = &self.mode else {
            return div().into_any_element();
        };
        if main.workspace_mode == WorkspaceMode::Todos {
            return self.render_todo_list(cx);
        }
        let selected = main.selected_id.clone();
        let list_width = main.list_width;
        let filter = main.filter.clone();
        let memos = self.visible_memos();
        let in_trash = matches!(filter, NavFilter::Trash);

        let mut list = v_flex()
            .w(list_width)
            .h_full()
            .border_r_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().panel_background)
            .id("memo-list")
            .overflow_y_scroll()
            .p_2()
            .gap_1();

        if memos.is_empty() {
            let focus = self.focus_handle(cx);
            let new_memo_binding = KeyBinding::for_action_in(&NewMemo, &focus, cx);
            let empty_label = if in_trash {
                "Trash is empty"
            } else {
                "No memos"
            };
            list = list.justify_center().items_center().child(
                v_flex()
                    .w(px(180.))
                    .max_w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        Label::new(empty_label)
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .when(!in_trash, |this| {
                        this.child(
                            Button::new("list-empty-new-memo", "New Memo")
                                .full_width()
                                .key_binding(new_memo_binding)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.create_memo(window, cx);
                                })),
                        )
                    }),
            );
            return list.into_any_element();
        }

        for (memo, sync) in memos {
            let id = memo.id.clone();
            let is_sel = selected.as_deref() == Some(id.as_str());
            let title = memo.display_title().to_string();
            let excerpt = memo.excerpt.clone();
            let memo_tags = memo.tags.clone();
            let row_id = SharedString::from(format!("memo-{id}"));
            let drag = DraggedMemo {
                id: id.clone(),
                title: title.clone().into(),
                anchor: None,
            };
            let menu_id = id.clone();
            let card = MemoCardData {
                title,
                excerpt,
                tags: memo_tags,
                notebook_name: None,
                is_pinned: memo.is_pinned,
                updated_at: Some(memo.updated_at.clone()),
                highlight_query: None,
            };
            list =
                list.child(
                    div()
                        .id(SharedString::from(format!("memo-drag-{id}")))
                        .w_full()
                        .on_drag(drag, |drag, position, _, cx| {
                            cx.new(|_| {
                                let mut d = drag.clone();
                                d.anchor = Some(position);
                                d
                            })
                        })
                        .on_mouse_down(gpui::MouseButton::Right, {
                            let entity = cx.weak_entity();
                            let menu_id = menu_id.clone();
                            move |event: &gpui::MouseDownEvent, window, cx| {
                                let position = event.position;
                                let menu_id = menu_id.clone();
                                entity
                                    .update(cx, |this, cx| {
                                        this.deploy_memo_menu(menu_id, position, window, cx);
                                    })
                                    .ok();
                            }
                        })
                        .child(memo_list_item(row_id, card, sync, is_sel, cx).on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.open_memo(&id, window, cx);
                            }),
                        )),
                );
        }

        list.into_any_element()
    }

    fn render_todo_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Mode::Main(main) = &self.mode else {
            return div().into_any_element();
        };
        let width = main.list_width;
        let selected = main.selected_todo_id.clone();
        let in_trash = main.todo_filter == TodoFilter::Trash;
        let todos = self.visible_todos();
        let mut list = v_flex()
            .w(width)
            .h_full()
            .border_r_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().panel_background)
            .id("todo-list")
            .p_2()
            .gap_1();
        if todos.is_empty() {
            let focus = self.focus_handle(cx);
            let new_todo_binding = KeyBinding::for_action_in(&NewTodo, &focus, cx);
            let empty_label = if in_trash {
                "Trash is empty"
            } else {
                "No tasks"
            };
            return list
                .justify_center()
                .items_center()
                .child(
                    v_flex()
                        .w(px(180.))
                        .max_w_full()
                        .gap_2()
                        .items_center()
                        .child(
                            Label::new(empty_label)
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        )
                        .when(!in_trash, |this| {
                            this.child(
                                Button::new("list-empty-new-todo", "New Task")
                                    .full_width()
                                    .key_binding(new_todo_binding)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.create_todo(window, cx);
                                    })),
                            )
                        }),
                )
                .into_any_element();
        }
        for (todo, _) in todos {
            let id = todo.id.clone();
            let complete_id = id.clone();
            let selected_id = id.clone();
            let menu_id = id.clone();
            let done = todo.status == panda_core::TodoStatus::Completed;
            let due = todo.due_date.clone().unwrap_or_default();
            let priority = todo.priority.clamp(0, 3);
            list = list.child(
                div()
                    .id(SharedString::from(format!("todo-{id}")))
                    .w_full()
                    .rounded_md()
                    .p_2()
                    .cursor_pointer()
                    .when(selected.as_deref() == Some(id.as_str()), |this| {
                        this.bg(cx.theme().colors().ghost_element_selected)
                    })
                    .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                    .on_mouse_down(gpui::MouseButton::Right, {
                        let entity = cx.weak_entity();
                        move |event: &gpui::MouseDownEvent, window, cx| {
                            let position = event.position;
                            let menu_id = menu_id.clone();
                            entity
                                .update(cx, |this, cx| {
                                    this.deploy_todo_menu(menu_id, position, window, cx);
                                })
                                .ok();
                        }
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .id(SharedString::from(format!("todo-check-{id}")))
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
                                    .hover(|style| {
                                        style.bg(cx.theme().colors().ghost_element_hover)
                                    })
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
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation();
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.toggle_todo(complete_id.clone(), window, cx);
                                    })),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .gap_1()
                                            .items_center()
                                            .child(
                                                div().flex_1().min_w_0().child(
                                                    Label::new(todo.title)
                                                        .size(LabelSize::Small)
                                                        .truncate()
                                                        .when(done, |label| {
                                                            label.color(Color::Muted)
                                                        }),
                                                ),
                                            )
                                            .children(todo_priority_badge(priority, cx)),
                                    )
                                    .when(!due.is_empty(), |content| {
                                        content.child(
                                            Label::new(format!("Due {due}"))
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted),
                                        )
                                    }),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_todo(&selected_id, window, cx);
                    })),
            );
        }
        list.into_any_element()
    }

    pub(crate) fn deploy_todo_menu(
        &mut self,
        todo_id: String,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Mode::Main(main) = &mut self.mode {
            main.selected_todo_id = Some(todo_id.clone());
        }
        let in_trash = matches!(
            &self.mode,
            Mode::Main(main) if main.todo_filter == panda_core::TodoFilter::Trash
        );
        let entity = cx.weak_entity();
        let menu = ContextMenu::build(window, cx, move |menu, _, _| {
            let entity = entity.clone();
            let id = todo_id.clone();
            if in_trash {
                menu.entry("Restore", None, {
                    let entity = entity.clone();
                    let id = id.clone();
                    move |window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                if let Mode::Main(main) = &mut this.mode {
                                    main.selected_todo_id = Some(id.clone());
                                }
                                this.restore_selected_todo(window, cx);
                            })
                            .ok();
                    }
                })
                .separator()
                .entry("Delete permanently", None, {
                    let entity = entity.clone();
                    move |window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                if let Mode::Main(main) = &mut this.mode {
                                    main.selected_todo_id = Some(id.clone());
                                }
                                this.permanently_delete_selected_todo(window, cx);
                            })
                            .ok();
                    }
                })
            } else {
                menu.entry("Delete", None, {
                    let entity = entity.clone();
                    move |window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                if let Mode::Main(main) = &mut this.mode {
                                    main.selected_todo_id = Some(id.clone());
                                }
                                this.delete_selected_todo(window, cx);
                            })
                            .ok();
                    }
                })
            }
        });
        self.set_context_menu(menu, position, window, cx);
    }

    pub(crate) fn deploy_memo_menu(
        &mut self,
        memo_id: String,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Mode::Main(main) = &mut self.mode {
            main.selected_id = Some(memo_id.clone());
        }
        let in_trash = matches!(
            &self.mode,
            Mode::Main(main) if matches!(main.filter, NavFilter::Trash)
        );
        let entity = cx.weak_entity();
        let pinned = matches!(
            &self.mode,
            Mode::Main(main)
                if main
                    .memos
                    .iter()
                    .any(|(m, _)| m.id == memo_id && m.is_pinned)
        );
        let pin_label = if pinned { "Unpin" } else { "Pin" };
        let menu = ContextMenu::build(window, cx, move |menu, _, _| {
            let entity = entity.clone();
            let id = memo_id.clone();
            if in_trash {
                menu.entry("Restore", None, {
                    let entity = entity.clone();
                    let id = id.clone();
                    move |window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                if let Mode::Main(main) = &mut this.mode {
                                    main.selected_id = Some(id.clone());
                                }
                                this.restore_selected(window, cx);
                            })
                            .ok();
                    }
                })
                .separator()
                .entry("Delete permanently", None, {
                    let entity = entity.clone();
                    move |window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                if let Mode::Main(main) = &mut this.mode {
                                    main.selected_id = Some(id.clone());
                                }
                                this.permanently_delete_selected(window, cx);
                            })
                            .ok();
                    }
                })
            } else {
                let id_rename = id.clone();
                let id_delete = id.clone();
                let id_pin = id;
                menu.entry(pin_label, None, {
                    let entity = entity.clone();
                    move |window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                this.toggle_pin_memo(id_pin.clone(), window, cx);
                            })
                            .ok();
                    }
                })
                .entry("Rename", None, {
                    let entity = entity.clone();
                    move |window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                this.begin_rename_memo(id_rename.clone(), window, cx);
                            })
                            .ok();
                    }
                })
                .separator()
                .entry("Delete", None, {
                    let entity = entity.clone();
                    move |window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                if let Mode::Main(main) = &mut this.mode {
                                    main.selected_id = Some(id_delete.clone());
                                }
                                this.delete_selected(window, cx);
                            })
                            .ok();
                    }
                })
            }
        });
        self.set_context_menu(menu, position, window, cx);
    }
}

fn todo_priority_badge(priority: i64, cx: &gpui::App) -> Option<AnyElement> {
    let (label, background, color) = match priority {
        1 => ("Low", cx.theme().status().info_background, Color::Accent),
        2 => (
            "Medium",
            cx.theme().status().warning_background,
            Color::Warning,
        ),
        3 => ("High", cx.theme().status().error_background, Color::Error),
        _ => return None,
    };
    Some(
        h_flex()
            .h(px(18.))
            .px_1p5()
            .rounded_sm()
            .bg(background)
            .items_center()
            .child(Label::new(label).size(LabelSize::XSmall).color(color))
            .into_any_element(),
    )
}
