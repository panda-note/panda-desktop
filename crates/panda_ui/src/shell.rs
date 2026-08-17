use std::sync::Arc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, KeyContext, MouseButton, SharedString,
    Subscription, Window, anchored, deferred, div, prelude::*, px,
};
use language::LanguageRegistry;
use panda_session::SessionStore;
use platform_title_bar::PlatformTitleBar;
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{h_flex, v_flex};
use vim::ModeIndicator;

use crate::app_menu::AppMenuBar;
use crate::chrome::cached_flex_child_style;
use crate::command_palette::PandaCommandPalette;
use crate::state::{DraggedPane, MAX_PANE_WIDTH, MIN_PANE_WIDTH, Mode, RenameTarget, ResizePane};

pub struct AppShell {
    pub(crate) languages: Arc<LanguageRegistry>,
    pub(crate) session: SessionStore,
    pub(crate) mode: Mode,
    pub(crate) title_bar: Option<Entity<PlatformTitleBar>>,
    pub(crate) menu_bar: Entity<AppMenuBar>,
    pub(crate) vim_indicator: Option<Entity<ModeIndicator>>,
    pub(crate) status_bar_visible: bool,
    pub(crate) session_load_error: Option<SharedString>,
    pub(crate) command_palette: Option<Entity<PandaCommandPalette>>,
    pub(crate) go_to_line: Option<Entity<go_to_line::GoToLine>>,
    pub(crate) _palette_sub: Option<Subscription>,
    pub(crate) _go_to_line_sub: Option<Subscription>,
    focus_handle: FocusHandle,
}

impl AppShell {
    pub fn new(
        languages: Arc<LanguageRegistry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut session_load_error = None;
        let session = SessionStore::load().unwrap_or_else(|e| {
            session_load_error = Some(e.to_string().into());
            eprintln!("session load failed: {e}");
            SessionStore::empty()
        });

        let title_bar = Some(Self::create_platform_title_bar(cx));
        let focus_handle = cx.focus_handle();
        let menu_bar = cx.new(|cx| {
            let mut bar = AppMenuBar::new(cx);
            bar.set_action_context(focus_handle.clone());
            bar
        });
        let vim_indicator = Some(Self::create_vim_indicator(window, cx));

        let mut shell = Self {
            languages: languages.clone(),
            session,
            mode: Mode::Setup(Self::blank_setup(window, cx)),
            title_bar,
            menu_bar,
            vim_indicator,
            status_bar_visible: true,
            session_load_error,
            command_palette: None,
            go_to_line: None,
            _palette_sub: None,
            _go_to_line_sub: None,
            focus_handle,
        };

        if let Some(instance) = shell.session.active().cloned() {
            shell.enter_main(instance, window, cx);
        } else if let Some(first) = shell.session.instances().first().cloned() {
            let _ = shell.session.set_active(&first.id);
            shell.enter_main(first, window, cx);
        } else {
            let handle = window.window_handle().downcast::<AppShell>();
            if let Some(handle) = handle {
                cx.defer(move |cx| {
                    crate::instances_window::InstancesWindow::open(handle, cx);
                });
            }
        }
        shell
    }

    pub(crate) fn render_resize_handle(
        &self,
        id: &'static str,
        pane: ResizePane,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id(id)
            .w(px(4.))
            .h_full()
            .cursor_col_resize()
            .occlude()
            .hover(|style| style.bg(cx.theme().colors().border))
            .on_drag(DraggedPane(pane), |drag, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .into_any_element()
    }

    pub(crate) fn render_side_chrome(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Mode::Main(main) = &self.mode else {
            return div().into_any_element();
        };
        let nav_collapsed = main.nav_collapsed;
        let mut panes = h_flex().h_full();
        if !nav_collapsed {
            panes = panes
                .child(self.render_nav_pane(window, cx))
                .child(self.render_resize_handle("resize-nav", ResizePane::Nav, cx));
        }
        panes
            .child(self.render_memo_list(window, cx))
            .child(self.render_resize_handle("resize-list", ResizePane::List, cx))
            .into_any_element()
    }

    fn handle_pane_drag_move(
        &mut self,
        event: &gpui::DragMoveEvent<DraggedPane>,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let x = event.event.position.x;
        match event.drag(cx).0 {
            ResizePane::Nav => {
                main.nav_width = x.clamp(px(MIN_PANE_WIDTH), px(MAX_PANE_WIDTH));
            }
            ResizePane::List => {
                let list_x = x - main.nav_width - px(4.);
                main.list_width = list_x.clamp(px(MIN_PANE_WIDTH), px(MAX_PANE_WIDTH));
            }
        }
        main.persist_pane_widths();
        cx.notify();
    }
}

impl Focusable for AppShell {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // PlatformTitleBar consumes children via mem::take on each paint; replenish
        // them here (same pattern as Zed's TitleBar) so the app menu cannot vanish
        // when AppShell is re-rendered via the ancestor dirty walk without notify.
        self.sync_title_bar(cx);

        let todo_workspace = matches!(
            &self.mode,
            Mode::Main(main) if main.workspace_mode == crate::state::WorkspaceMode::Todos
        );

        let content = match &self.mode {
            Mode::Setup(_) => self.render_setup(window, cx),
            Mode::Main(main) => {
                let nav_collapsed = main.nav_collapsed;
                let side_width = {
                    let list = main.list_width + px(4.);
                    if nav_collapsed {
                        list
                    } else {
                        main.nav_width + px(4.) + list
                    }
                };
                let side_chrome = main.side_chrome.clone();
                let editor_chrome = main.editor_chrome.clone();
                let error = self.render_error_banner(cx);
                let context_menu = main.context_menu.as_ref().map(|(menu, position, _)| {
                    deferred(
                        anchored()
                            .position(*position)
                            .anchor(gpui::Anchor::TopLeft)
                            .child(div().occlude().child(menu.clone())),
                    )
                    .with_priority(1)
                });
                let rename = main.rename_dialog.as_ref().map(|dialog| {
                    let label = match &dialog.target {
                        RenameTarget::Memo(_) => "Rename memo",
                        RenameTarget::Notebook(_) => "Rename folder",
                    };
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_start()
                        .justify_center()
                        .pt(px(120.))
                        .bg(gpui::hsla(0., 0., 0., 0.25))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.cancel_rename(cx)),
                        )
                        .child(
                            v_flex()
                                .w(px(360.))
                                .gap_2()
                                .p_3()
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().colors().border)
                                .bg(cx.theme().colors().elevated_surface_background)
                                .occlude()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_action(cx.listener(|this, _: &menu::Confirm, window, cx| {
                                    this.confirm_rename(window, cx);
                                }))
                                .on_action(cx.listener(|this, _: &menu::Cancel, _, cx| {
                                    this.cancel_rename(cx);
                                }))
                                .child(Label::new(label).size(LabelSize::Small))
                                .child(
                                    div()
                                        .h(px(32.))
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .px_2()
                                        .border_1()
                                        .border_color(cx.theme().colors().border)
                                        .rounded_md()
                                        .child(
                                            div()
                                                .w_full()
                                                .h_full()
                                                .flex()
                                                .items_center()
                                                .child(dialog.editor.clone()),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .justify_end()
                                        .gap_2()
                                        .child(Button::new("rename-cancel", "Cancel").on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.cancel_rename(cx);
                                            }),
                                        ))
                                        .child(Button::new("rename-ok", "Rename").on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.confirm_rename(window, cx);
                                            }),
                                        )),
                                ),
                        )
                });

                let mut panes = h_flex().size_full().on_drag_move(cx.listener(
                    |this, e: &gpui::DragMoveEvent<DraggedPane>, _, cx| {
                        this.handle_pane_drag_move(e, cx);
                    },
                ));
                if let Some(side) = side_chrome {
                    panes = panes.child(side.cached(
                        gpui::StyleRefinement::default()
                            .h_full()
                            .w(side_width)
                            .flex_none(),
                    ));
                }
                if let Some(editor) = editor_chrome {
                    panes = panes.child(editor.cached(cached_flex_child_style()));
                }

                v_flex()
                    .size_full()
                    .relative()
                    .children(error)
                    .child(panes)
                    .children(context_menu)
                    .children(rename)
                    .into_any_element()
            }
        };

        let overlays = if self.command_palette.is_some() || self.go_to_line.is_some() {
            Some(
                div()
                    .absolute()
                    .inset_0()
                    .children(self.command_palette.clone())
                    .children(self.go_to_line.clone().map(|modal| {
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_start()
                            .justify_center()
                            .pt(px(120.))
                            .bg(gpui::hsla(0., 0., 0., 0.25))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.go_to_line = None;
                                    this._go_to_line_sub = None;
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .occlude()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .child(modal),
                            )
                    })),
            )
        } else {
            None
        };

        let mut key_context = KeyContext::new_with_defaults();
        key_context.add("Workspace");
        if todo_workspace {
            key_context.add("Todos");
        }

        v_flex()
            .size_full()
            .key_context(key_context)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(
                |this, _: &crate::panda_actions::ToggleCommandPalette, window, cx| {
                    this.toggle_command_palette(window, cx);
                },
            ))
            .on_action(cx.listener(
                |this, _: &zed_actions::command_palette::Toggle, window, cx| {
                    this.toggle_command_palette(window, cx);
                },
            ))
            .on_action(
                cx.listener(|this, _: &crate::panda_actions::NewMemo, window, cx| {
                    if matches!(&this.mode, crate::state::Mode::Main(main) if main.workspace_mode == crate::state::WorkspaceMode::Todos)
                    {
                        this.create_todo(window, cx);
                    } else {
                        this.create_memo(window, cx);
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::panda_actions::NewTodo, window, cx| {
                    this.create_todo(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::panda_actions::SaveMemo, window, cx| {
                    if matches!(&this.mode, crate::state::Mode::Main(main) if main.workspace_mode == crate::state::WorkspaceMode::Todos) {
                        this.save_todo(window, cx);
                    } else {
                        this.save_memo_now(cx);
                    }
                }),
            )
            .on_action(cx.listener(|this, _: &workspace::Save, window, cx| {
                if matches!(&this.mode, crate::state::Mode::Main(main) if main.workspace_mode == crate::state::WorkspaceMode::Todos) {
                    this.save_todo(window, cx);
                } else {
                    this.save_memo_now(cx);
                }
            }))
            .on_action(
                cx.listener(|this, _: &crate::panda_actions::DeleteMemo, window, cx| {
                    this.delete_selected(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::panda_actions::Refresh, window, cx| {
                    this.refresh_remote(window, cx);
                }),
            )
            .on_action(cx.listener(
                |this, _: &crate::panda_actions::OpenInstances, window, cx| {
                    this.open_instances_window(window, cx);
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::panda_actions::OpenSettings, _window, cx| {
                    this.open_settings_window(_window, cx);
                },
            ))
            .on_action(
                cx.listener(|this, _: &zed_actions::OpenSettings, window, cx| {
                    this.open_settings_window(window, cx);
                }),
            )
            .on_action(
                cx.listener(|_this, _: &crate::panda_actions::OpenAbout, _window, cx| {
                    crate::about_window::AboutWindow::open(cx);
                }),
            )
            .on_action(
                cx.listener(|_this, _: &crate::panda_actions::Quit, _window, cx| {
                    cx.quit();
                }),
            )
            .on_action(cx.listener(
                |this, _: &crate::panda_actions::TogglePreview, _window, cx| {
                    this.toggle_preview(cx);
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::panda_actions::ToggleStatusBar, _window, cx| {
                    this.toggle_status_bar(cx);
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::panda_actions::ToggleNavPane, _window, cx| {
                    this.toggle_nav_pane(cx);
                },
            ))
            .on_action(
                cx.listener(|this, _: &crate::panda_actions::FormatMemo, window, cx| {
                    this.format_memo(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::panda_actions::GoToLine, window, cx| {
                    this.open_go_to_line(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &editor::actions::ToggleGoToLine, window, cx| {
                    this.open_go_to_line(window, cx);
                }),
            )
            .bg(cx.theme().colors().background)
            .text_color(cx.theme().colors().text)
            .children(self.title_bar.clone())
            .children(self.session_load_error.clone().map(|e| {
                div()
                    .px_3()
                    .py_1()
                    .bg(cx.theme().status().error_background)
                    .child(Label::new(e).size(LabelSize::Small).color(Color::Error))
            }))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .child(content)
                    .children(overlays),
            )
            .children(self.render_status_bar(window, cx))
    }
}
