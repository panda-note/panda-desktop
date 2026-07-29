use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, Context, Entity, Focusable, MouseButton, Point, Window, div, prelude::*, px,
};
use panda_core::{NavFilter, Notebook, TodoFilter};
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{ContextMenu, Divider, DividerColor, ListItem, ListItemSpacing};

use crate::shell::AppShell;
use crate::state::{DraggedMemo, Mode, WorkspaceMode};

impl AppShell {
    pub(crate) fn render_nav_pane(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Mode::Main(main) = &self.mode else {
            return div().into_any_element();
        };
        if main.workspace_mode == WorkspaceMode::Todos {
            return self.render_todo_nav(cx);
        }
        let notebooks = main.notebooks.clone();
        let filter = main.filter.clone();
        let collapsed = main.collapsed_notebooks.clone();
        let nav_width = main.nav_width;
        let available_tags = main.available_tags.clone();

        let mut nav = v_flex()
            .w(nav_width)
            .h_full()
            .border_r_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().element_background)
            .p_1()
            .gap_0p5()
            .id("nav-pane")
            .overflow_y_scroll()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.deploy_nav_blank_menu(event.position, window, cx);
                }),
            )
            .child(self.render_workspace_switch(WorkspaceMode::Memos, cx))
            .child(Divider::horizontal().color(DividerColor::BorderFaded))
            .child(
                div()
                    .w_full()
                    .mt_1()
                    .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                    .child(nav_item(
                        "nav-all",
                        IconName::FileMultiple,
                        "All Notes",
                        matches!(filter, NavFilter::All),
                        0,
                        None,
                        None,
                        cx,
                        |this, _, _, cx| {
                            if let Mode::Main(main) = &mut this.mode {
                                main.filter = NavFilter::All;
                            }
                            cx.notify();
                        },
                    )),
            )
            .child(
                div()
                    .w_full()
                    .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                    .child(nav_item(
                        "nav-pinned",
                        IconName::Pin,
                        "Pinned",
                        matches!(filter, NavFilter::Pinned),
                        0,
                        None,
                        None,
                        cx,
                        |this, _, _, cx| {
                            if let Mode::Main(main) = &mut this.mode {
                                main.filter = NavFilter::Pinned;
                            }
                            cx.notify();
                        },
                    )),
            )
            .child(
                h_flex()
                    .px_2()
                    .pt_2()
                    .pb_1()
                    .justify_between()
                    .child(
                        Label::new("Notebooks")
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    )
                    .child(
                        IconButton::new("nav-new-root", IconName::Plus)
                            .icon_size(IconSize::XSmall)
                            .style(ButtonStyle::Subtle)
                            .tooltip(ui::Tooltip::text("New folder"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_notebook("New Folder".into(), None, window, cx);
                            })),
                    ),
            );

        for (nb, depth) in notebook_tree_rows(&notebooks, &collapsed) {
            let id = nb.id.clone();
            let selected_nb = matches!(&filter, NavFilter::Notebook(x) if *x == id);
            let name = nb.name.clone();
            let btn_id = format!("nb-{id}");
            let has_children = notebooks
                .iter()
                .any(|c| c.parent_id.as_deref() == Some(id.as_str()));
            let expanded = !collapsed.contains(&id);
            let is_inbox = nb.slug.as_deref() == Some("inbox");
            let disclosure = if has_children {
                let toggle_id = id.clone();
                let icon = if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                };
                Some(
                    IconButton::new(format!("nb-toggle-{id}"), icon)
                        .icon_size(IconSize::XSmall)
                        .style(ButtonStyle::Subtle)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Mode::Main(main) = &mut this.mode {
                                if main.collapsed_notebooks.contains(&toggle_id) {
                                    main.collapsed_notebooks.remove(&toggle_id);
                                } else {
                                    main.collapsed_notebooks.insert(toggle_id.clone());
                                }
                            }
                            cx.notify();
                        })),
                )
            } else {
                None
            };
            let drop_id = id.clone();
            let menu_id = id.clone();
            nav = nav.child(
                div()
                    .id(SharedString::from(format!("nb-drop-{id}")))
                    .w_full()
                    .can_drop(|drag: &dyn std::any::Any, _, _| drag.is::<DraggedMemo>())
                    .on_drop(cx.listener(move |this, drag: &DraggedMemo, window, cx| {
                        this.move_memo_to_notebook(drag.id.clone(), drop_id.clone(), window, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.deploy_notebook_menu(
                                menu_id.clone(),
                                is_inbox,
                                event.position,
                                window,
                                cx,
                            );
                        }),
                    )
                    .child(nav_item(
                        btn_id,
                        IconName::Book,
                        name,
                        selected_nb,
                        depth,
                        disclosure,
                        Some(id.clone()),
                        cx,
                        move |this, _, _, cx| {
                            if let Mode::Main(main) = &mut this.mode {
                                main.filter = NavFilter::Notebook(id.clone());
                            }
                            cx.notify();
                        },
                    )),
            );
        }

        if !available_tags.is_empty() {
            nav = nav.child(
                h_flex()
                    .px_2()
                    .pt_2()
                    .pb_1()
                    .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                    .child(
                        Label::new("Tags")
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    ),
            );
            nav = nav.child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .gap_1()
                    .px_2()
                    .on_mouse_down(MouseButton::Right, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .children(available_tags.into_iter().map(|tag| {
                        let selected_tag = matches!(
                            &filter,
                            NavFilter::Tag(name) if name.eq_ignore_ascii_case(&tag)
                        );
                        let tag_name = tag.clone();
                        let text_color = if selected_tag {
                            Color::Accent
                        } else {
                            Color::Muted
                        };
                        div()
                            .id(SharedString::from(format!("tag-{tag}")))
                            .h(px(24.))
                            .max_w(px(150.))
                            .min_w_0()
                            .px_2()
                            .gap_1()
                            .rounded_sm()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .bg(if selected_tag {
                                cx.theme().colors().text_accent.opacity(0.16)
                            } else {
                                cx.theme().colors().ghost_element_background
                            })
                            .hover(|style| style.bg(cx.theme().colors().element_hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Mode::Main(main) = &mut this.mode {
                                    main.filter = NavFilter::Tag(tag_name.clone());
                                }
                                cx.notify();
                            }))
                            .child(
                                Icon::new(IconName::Hash)
                                    .size(IconSize::XSmall)
                                    .color(text_color),
                            )
                            .child(
                                Label::new(tag)
                                    .size(LabelSize::XSmall)
                                    .color(text_color)
                                    .truncate(),
                            )
                    })),
            );
        }

        nav = nav.child(
            div()
                .w_full()
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .child(nav_item(
                    "nav-trash",
                    IconName::Trash,
                    "Trash",
                    matches!(filter, NavFilter::Trash),
                    0,
                    None,
                    None,
                    cx,
                    |this, _, _, cx| {
                        if let Mode::Main(main) = &mut this.mode {
                            main.filter = NavFilter::Trash;
                        }
                        cx.notify();
                    },
                )),
        );

        nav.into_any_element()
    }

    fn render_todo_nav(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Mode::Main(main) = &self.mode else {
            return div().into_any_element();
        };
        let width = main.nav_width;
        let selected_filter = main.todo_filter;
        let todos = main.todos.clone();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let count = |filter: TodoFilter| {
            let mut seen_todo_ids = HashSet::new();
            todos
                .iter()
                .filter(|(todo, _)| {
                    if filter == TodoFilter::Trash {
                        todo.is_deleted
                    } else {
                        !todo.is_deleted
                    }
                })
                .filter(|(todo, _)| match filter {
                    TodoFilter::All => true,
                    TodoFilter::Inbox => {
                        todo.status != panda_core::TodoStatus::Completed && todo.due_date.is_none()
                    }
                    TodoFilter::Today => {
                        todo.status != panda_core::TodoStatus::Completed
                            && todo.due_date.as_deref() == Some(today.as_str())
                    }
                    TodoFilter::Upcoming => {
                        todo.status != panda_core::TodoStatus::Completed
                            && todo
                                .due_date
                                .as_deref()
                                .is_some_and(|due| due > today.as_str())
                    }
                    TodoFilter::Completed => todo.status == panda_core::TodoStatus::Completed,
                    TodoFilter::Trash => true,
                })
                .filter(|(todo, _)| seen_todo_ids.insert(todo.id.clone()))
                .count()
        };
        let mut nav = v_flex()
            .w(width)
            .h_full()
            .border_r_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().element_background)
            .p_1()
            .gap_0p5()
            .id("nav-pane")
            .overflow_y_scroll()
            .child(self.render_workspace_switch(WorkspaceMode::Todos, cx))
            .child(Divider::horizontal().color(DividerColor::BorderFaded))
            .child(div().mt_1());
        for (filter, label, icon) in [
            (TodoFilter::All, "All Tasks", IconName::FileMultiple),
            (TodoFilter::Inbox, "Inbox", IconName::File),
            (TodoFilter::Today, "Today", IconName::FileMultiple),
            (TodoFilter::Upcoming, "Upcoming", IconName::FileMultiple),
            (TodoFilter::Completed, "Completed", IconName::Check),
            (TodoFilter::Trash, "Trash", IconName::Trash),
        ] {
            let text = format!("{} ({})", label, count(filter));
            nav = nav.child(nav_item(
                format!("todo-filter-{label}"),
                icon,
                text,
                selected_filter == filter,
                0,
                None,
                None,
                cx,
                move |this, _, _, cx| this.select_todo_filter(filter, cx),
            ));
        }
        nav.into_any_element()
    }

    fn render_workspace_switch(
        &mut self,
        selected: WorkspaceMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let button = |id, label, icon, mode, is_selected| {
            div()
                .flex_1()
                .rounded_sm()
                .when(is_selected, |this| {
                    this.bg(cx.theme().colors().element_hover)
                })
                .child(
                    Button::new(id, label)
                        .full_width()
                        .style(ButtonStyle::Transparent)
                        .start_icon(Icon::new(icon).size(IconSize::Small))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_workspace(mode, cx);
                        })),
                )
        };
        h_flex()
            .w_full()
            .gap_1()
            .p_0p5()
            .rounded_md()
            .bg(cx.theme().colors().ghost_element_background)
            .child(button(
                "workspace-memos",
                "Notes",
                IconName::FileMultiple,
                WorkspaceMode::Memos,
                selected == WorkspaceMode::Memos,
            ))
            .child(button(
                "workspace-todos",
                "Todos",
                IconName::ListTodo,
                WorkspaceMode::Todos,
                selected == WorkspaceMode::Todos,
            ))
            .into_any_element()
    }

    pub(crate) fn deploy_nav_blank_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.weak_entity();
        let menu = ContextMenu::build(window, cx, move |menu, _, _| {
            menu.entry("New Folder", None, {
                let entity = entity.clone();
                move |window, cx| {
                    entity
                        .update(cx, |this, cx| {
                            this.create_notebook("New Folder".into(), None, window, cx);
                        })
                        .ok();
                }
            })
        });
        self.set_context_menu(menu, position, window, cx);
    }

    pub(crate) fn deploy_notebook_menu(
        &mut self,
        notebook_id: String,
        is_inbox: bool,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.weak_entity();
        let menu = ContextMenu::build(window, cx, move |menu, _, _| {
            let entity = entity.clone();
            let id_child = notebook_id.clone();
            let id_rename = notebook_id.clone();
            let id_delete = notebook_id.clone();
            menu.entry("New Subfolder", None, {
                let entity = entity.clone();
                move |window, cx| {
                    entity
                        .update(cx, |this, cx| {
                            this.create_notebook(
                                "New Folder".into(),
                                Some(id_child.clone()),
                                window,
                                cx,
                            );
                        })
                        .ok();
                }
            })
            .separator()
            .entry("Rename", None, {
                let entity = entity.clone();
                move |window, cx| {
                    entity
                        .update(cx, |this, cx| {
                            this.begin_rename_notebook(id_rename.clone(), window, cx);
                        })
                        .ok();
                }
            })
            .when(!is_inbox, |menu| {
                menu.entry("Delete", None, {
                    let entity = entity.clone();
                    move |window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                this.delete_notebook(id_delete.clone(), window, cx);
                            })
                            .ok();
                    }
                })
            })
        });
        self.set_context_menu(menu, position, window, cx);
    }

    pub(crate) fn set_context_menu(
        &mut self,
        menu: Entity<ContextMenu>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&menu.focus_handle(cx), cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &gpui::DismissEvent, cx| {
            if let Mode::Main(main) = &mut this.mode {
                main.context_menu = None;
            }
            cx.notify();
        });
        if let Mode::Main(main) = &mut self.mode {
            main.context_menu = Some((menu, position, subscription));
        }
        cx.notify();
    }
}

fn notebook_tree_rows<'a>(
    notebooks: &'a [Notebook],
    collapsed: &std::collections::HashSet<String>,
) -> Vec<(&'a Notebook, i32)> {
    let mut children: HashMap<Option<&str>, Vec<&Notebook>> = HashMap::new();
    for nb in notebooks {
        children
            .entry(nb.parent_id.as_deref())
            .or_default()
            .push(nb);
    }
    for list in children.values_mut() {
        list.sort_by(|a, b| {
            a.sort_order
                .cmp(&b.sort_order)
                .then_with(|| a.name.cmp(&b.name))
        });
    }

    let mut out = Vec::new();
    fn walk<'a>(
        parent: Option<&str>,
        depth: i32,
        children: &HashMap<Option<&str>, Vec<&'a Notebook>>,
        collapsed: &std::collections::HashSet<String>,
        out: &mut Vec<(&'a Notebook, i32)>,
    ) {
        let Some(list) = children.get(&parent) else {
            return;
        };
        for nb in list {
            out.push((nb, depth));
            if !collapsed.contains(&nb.id) {
                walk(Some(nb.id.as_str()), depth + 1, children, collapsed, out);
            }
        }
    }
    walk(None, 0, &children, collapsed, &mut out);
    out
}

fn nav_item(
    id: impl Into<gpui::ElementId>,
    icon: IconName,
    label: impl Into<gpui::SharedString>,
    selected: bool,
    depth: i32,
    disclosure: Option<IconButton>,
    _notebook_id: Option<String>,
    cx: &mut Context<AppShell>,
    on_click: impl Fn(&mut AppShell, &gpui::ClickEvent, &mut Window, &mut Context<AppShell>) + 'static,
) -> impl IntoElement {
    let label = label.into();
    let indent = px((depth.max(0) as f32) * 12.);
    let spacer = disclosure.is_none() && depth > 0;
    ListItem::new(id)
        .spacing(ListItemSpacing::Dense)
        .inset(true)
        .toggle_state(selected)
        .start_slot(
            h_flex()
                .gap_0p5()
                .ml(indent)
                .children(disclosure)
                .when(spacer, |this| this.child(div().w(px(20.))))
                .child(Icon::new(icon).size(IconSize::Small).color(Color::Muted)),
        )
        .child(Label::new(label).size(LabelSize::Small))
        .on_click(cx.listener(on_click))
}
