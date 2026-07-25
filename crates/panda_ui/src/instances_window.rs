//! Independent Instances window (list / select / add / edit / remove).

use editor::Editor;
use gpui::{
    App, Bounds, Context, Entity, FocusHandle, Focusable, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions, point, prelude::*, px, size,
};
use panda_core::Instance;
use panda_session::{SessionStore, app_data_dir};
use platform_title_bar::PlatformTitleBar;
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{Banner, Divider, ListItem, Severity, Tooltip};
use workspace::client_side_decorations;

use crate::shell::AppShell;
use crate::widgets::{field_row, section_header};

pub struct InstancesWindow {
    main: WindowHandle<AppShell>,
    session: SessionStore,
    title_bar: Option<Entity<PlatformTitleBar>>,
    name: Entity<Editor>,
    api_url: Entity<Editor>,
    username: Entity<Editor>,
    password: Entity<Editor>,
    token: Entity<Editor>,
    /// When set, form edits this saved instance instead of creating a new one.
    editing_id: Option<String>,
    error: Option<SharedString>,
    busy: bool,
    focus_handle: FocusHandle,
}

impl InstancesWindow {
    pub fn open(main: WindowHandle<AppShell>, cx: &mut App) {
        if let Some(existing) = cx
            .windows()
            .into_iter()
            .find_map(|w| w.downcast::<InstancesWindow>())
        {
            existing
                .update(cx, |_this, window, _cx| {
                    window.activate_window();
                })
                .ok();
            return;
        }

        cx.defer(move |cx| {
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(640.), px(600.)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Panda Note — Instances".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(9.), px(9.))),
                    }),
                    focus: true,
                    show: true,
                    is_movable: true,
                    kind: gpui::WindowKind::Normal,
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| InstancesWindow::new(main, window, cx)),
            );
        });
    }

    fn new(main: WindowHandle<AppShell>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let session = SessionStore::load().unwrap_or_else(|e| {
            eprintln!("instances window session load: {e}");
            SessionStore::empty()
        });
        let title_bar = Some(cx.new(|cx| PlatformTitleBar::new("instances-title-bar", cx)));
        if let Some(bar) = &title_bar {
            bar.update(cx, |bar, _| {
                bar.set_children(vec![
                    h_flex()
                        .pl_2()
                        .child(Label::new("Instances").size(LabelSize::Small))
                        .into_any_element(),
                ]);
            });
        }
        Self {
            main,
            session,
            title_bar,
            name: field_editor(window, cx, "Local"),
            api_url: field_editor(window, cx, "http://127.0.0.1:8787/api/v1"),
            username: field_editor(window, cx, "admin"),
            password: field_editor(window, cx, "admin123"),
            token: field_editor(window, cx, ""),
            editing_id: None,
            error: None,
            busy: false,
            focus_handle: cx.focus_handle(),
        }
    }

    fn set_editor_text(
        editor: &Entity<Editor>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        editor.update(cx, |ed, cx| ed.set_text(text.to_string(), window, cx));
    }

    fn activate(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Err(e) = self.session.set_active(id) {
            self.error = Some(e.to_string().into());
            cx.notify();
            return;
        }
        let Some(instance) = self.session.active().cloned() else {
            return;
        };
        let main = self.main;
        let instance_id = instance.id.clone();
        cx.defer(move |cx| {
            main.update(cx, |shell, window, cx| {
                if let Ok(session) = SessionStore::load() {
                    shell.session = session;
                } else {
                    let _ = shell.session.set_active(&instance_id);
                }
                if let Some(instance) = shell.session.active().cloned() {
                    shell.enter_main(instance, window, cx);
                }
                window.activate_window();
            })
            .ok();
        });
        cx.notify();
    }

    fn remove(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.editing_id.as_deref() == Some(id) {
            self.editing_id = None;
        }
        if let Err(e) = self.session.remove(id) {
            self.error = Some(e.to_string().into());
        }
        cx.notify();
    }

    fn start_edit(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(inst) = self
            .session
            .instances()
            .iter()
            .find(|i| i.id == id)
            .cloned()
        else {
            return;
        };
        self.editing_id = Some(inst.id);
        Self::set_editor_text(&self.name, &inst.name, window, cx);
        Self::set_editor_text(&self.api_url, &inst.api_url, window, cx);
        Self::set_editor_text(&self.username, "", window, cx);
        Self::set_editor_text(&self.password, "", window, cx);
        Self::set_editor_text(&self.token, "", window, cx);
        self.error = None;
        cx.notify();
    }

    fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_id = None;
        Self::set_editor_text(&self.name, "Local", window, cx);
        Self::set_editor_text(&self.api_url, "http://127.0.0.1:8787/api/v1", window, cx);
        Self::set_editor_text(&self.username, "admin", window, cx);
        Self::set_editor_text(&self.password, "admin123", window, cx);
        Self::set_editor_text(&self.token, "", window, cx);
        self.error = None;
        cx.notify();
    }

    fn save_details(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.editing_id.clone() else {
            return;
        };
        let name = self.name.read(cx).text(cx);
        let api_url = self.api_url.read(cx).text(cx);
        match self
            .session
            .update_details(&id, name.trim(), api_url.trim())
        {
            Ok(()) => {
                self.error = None;
                self.activate(&id, cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn add_login(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        let name = self.name.read(cx).text(cx);
        let api_url = self.api_url.read(cx).text(cx);
        let username = self.username.read(cx).text(cx);
        let password = self.password.read(cx).text(cx);
        let result = if let Some(id) = self.editing_id.clone() {
            self.session.update_via_login(
                &id,
                name.trim(),
                api_url.trim(),
                username.trim(),
                password.trim(),
            )
        } else {
            self.session.add_via_login(
                name.trim(),
                api_url.trim(),
                username.trim(),
                password.trim(),
            )
        };
        match result {
            Ok(instance) => {
                self.busy = false;
                self.editing_id = None;
                self.activate(&instance.id, cx);
            }
            Err(e) => {
                self.busy = false;
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn add_token(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).text(cx);
        let api_url = self.api_url.read(cx).text(cx);
        let token = self.token.read(cx).text(cx);
        if token.trim().is_empty() {
            self.error = Some("Token is empty".into());
            cx.notify();
            return;
        }
        let result = if let Some(id) = self.editing_id.clone() {
            self.session
                .update_via_token(&id, name.trim(), api_url.trim(), token.trim())
        } else {
            self.session
                .add_via_token(name.trim(), api_url.trim(), token.trim())
        };
        match result {
            Ok(instance) => {
                self.editing_id = None;
                self.activate(&instance.id, cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }
}

impl Focusable for InstancesWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InstancesWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_id = self.session.active().map(|i| i.id.clone());
        let instances: Vec<Instance> = self.session.instances().to_vec();
        let error = self.error.clone();
        let busy = self.busy;
        let editing = self.editing_id.is_some();
        let data_dir = app_data_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "(unknown)".into());

        let mut list = v_flex().gap_1().w_full();
        if instances.is_empty() {
            list = list.child(
                Label::new("No saved instances yet")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            );
        }
        for inst in instances {
            let id = inst.id.clone();
            let id_edit = inst.id.clone();
            let id_remove = inst.id.clone();
            let selected = active_id.as_deref() == Some(id.as_str());
            let editing_this = self.editing_id.as_deref() == Some(id.as_str());
            list = list.child(
                ListItem::new(SharedString::from(format!("inst-{id}")))
                    .inset(true)
                    .toggle_state(selected || editing_this)
                    .start_slot(
                        Icon::new(IconName::Server)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .end_slot(
                        h_flex()
                            .gap_0p5()
                            .child(
                                IconButton::new(
                                    SharedString::from(format!("edit-{id_edit}")),
                                    IconName::Pencil,
                                )
                                .icon_size(IconSize::Small)
                                .style(ButtonStyle::Subtle)
                                .tooltip(Tooltip::text("Edit"))
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.start_edit(&id_edit, window, cx);
                                    },
                                )),
                            )
                            .child(
                                IconButton::new(
                                    SharedString::from(format!("rm-{id_remove}")),
                                    IconName::Trash,
                                )
                                .icon_size(IconSize::Small)
                                .style(ButtonStyle::Subtle)
                                .on_click(cx.listener(
                                    move |this, _, _window, cx| {
                                        this.remove(&id_remove, cx);
                                    },
                                )),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(Label::new(inst.name.clone()).size(LabelSize::Small))
                            .child(
                                Label::new(inst.api_url.clone())
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.activate(&id, cx);
                    })),
            );
        }

        let form_title = if editing {
            "Edit instance"
        } else {
            "Add instance"
        };
        let login_label = if busy {
            "Connecting..."
        } else if editing {
            "Save with Login"
        } else {
            "Login and Connect"
        };
        let token_label = if editing {
            "Save with Token"
        } else {
            "Connect with Token"
        };

        v_flex()
            .size_full()
            .text_color(cx.theme().colors().text)
            .children(self.title_bar.clone())
            .child(
                v_flex()
                    .flex_1()
                    .p_4()
                    .gap_4()
                    .bg(cx.theme().colors().editor_background)
                    .id("instances-scroll")
                    .overflow_y_scroll()
                    .child(Label::new("Instances").size(LabelSize::Large))
                    .child(section_header("Saved instances"))
                    .child(list)
                    .child(Divider::horizontal())
                    .child(section_header(form_title))
                    .child(field_row("Name", self.name.clone(), cx))
                    .child(field_row("API URL", self.api_url.clone(), cx))
                    .when(editing, |this| {
                        this.child(
                            Button::new("inst-save-details", "Save Name & URL")
                                .style(ButtonStyle::Subtle)
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.save_details(cx);
                                })),
                        )
                    })
                    .child(field_row("Username", self.username.clone(), cx))
                    .child(field_row("Password", self.password.clone(), cx))
                    .child(
                        Button::new("inst-login", login_label)
                            .style(ButtonStyle::Filled)
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _window, cx| this.add_login(cx))),
                    )
                    .child(Label::new("or paste API token").size(LabelSize::XSmall))
                    .child(field_row("Token", self.token.clone(), cx))
                    .child(
                        Button::new("inst-token", token_label)
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|this, _, _window, cx| this.add_token(cx))),
                    )
                    .when(editing, |this| {
                        this.child(
                            Button::new("inst-cancel-edit", "Cancel Edit")
                                .style(ButtonStyle::Subtle)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_edit(window, cx);
                                })),
                        )
                    })
                    .children(error.map(|e| {
                        Banner::new()
                            .severity(Severity::Warning)
                            .child(Label::new(e).size(LabelSize::Small))
                    }))
                    .child(
                        Label::new(format!("Data: {data_dir}"))
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    ),
            )
            .map(|root| client_side_decorations(root, _window, cx))
    }
}

fn field_editor(
    window: &mut Window,
    cx: &mut Context<InstancesWindow>,
    text: &str,
) -> Entity<Editor> {
    cx.new(|cx| {
        let mut editor = Editor::single_line(window, cx);
        editor.set_text(text.to_string(), window, cx);
        editor.set_show_gutter(false, cx);
        editor
    })
}
