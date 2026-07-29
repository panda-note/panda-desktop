//! Lightweight Settings window (not Zed settings_ui).

use editor::EditorSettings;
use gpui::{
    App, Bounds, Context, Entity, FocusHandle, Focusable, TitlebarOptions, UpdateGlobal, Window,
    WindowBounds, WindowHandle, WindowOptions, point, prelude::*, px, size,
};
use platform_title_bar::PlatformTitleBar;
use settings::{Settings, SettingsStore};
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{ContextMenu, Divider, DropdownMenu, SwitchField, ToggleState};
use vim_mode_setting::VimModeSetting;
use workspace::client_side_decorations;

use panda_session::app_data_dir;

use crate::shell::AppShell;

#[derive(Clone)]
struct PandaPrefs {
    vim: bool,
    line_numbers: bool,
    breakpoints: bool,
    theme: String,
}

pub struct SettingsWindow {
    title_bar: Option<Entity<PlatformTitleBar>>,
    focus_handle: FocusHandle,
}

impl SettingsWindow {
    pub fn open(cx: &mut App) {
        if let Some(existing) = cx
            .windows()
            .into_iter()
            .find_map(|w| w.downcast::<SettingsWindow>())
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
                        size(px(480.), px(540.)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Panda Note — Settings".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(9.), px(9.))),
                    }),
                    focus: true,
                    show: true,
                    is_movable: true,
                    kind: gpui::WindowKind::Normal,
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| SettingsWindow::new(window, cx)),
            );
        });
    }

    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title_bar = Some(cx.new(|cx| PlatformTitleBar::new("settings-title-bar", cx)));
        if let Some(bar) = &title_bar {
            bar.update(cx, |bar, _| {
                bar.set_children(vec![
                    h_flex()
                        .pl_2()
                        .child(Label::new("Settings").size(LabelSize::Small))
                        .into_any_element(),
                ]);
            });
        }
        Self {
            title_bar,
            focus_handle: cx.focus_handle(),
        }
    }

    fn current_prefs(cx: &App) -> PandaPrefs {
        let gutter = EditorSettings::get_global(cx).gutter;
        PandaPrefs {
            vim: VimModeSetting::get_global(cx).0,
            line_numbers: gutter.line_numbers,
            breakpoints: gutter.breakpoints,
            theme: cx.theme().name.to_string(),
        }
    }

    fn persist_prefs(cx: &mut App, prefs: PandaPrefs) {
        let json = format!(
            r#"{{
                "theme": "{theme}",
                "vim_mode": {vim},
                "base_keymap": "VSCode",
                "gutter": {{
                    "line_numbers": {line_numbers},
                    "breakpoints": {breakpoints},
                    "runnables": false,
                    "bookmarks": false
                }}
            }}"#,
            vim = prefs.vim,
            line_numbers = prefs.line_numbers,
            breakpoints = prefs.breakpoints,
            theme = prefs.theme,
        );
        SettingsStore::update_global(cx, |store, cx| {
            let _ = store.set_user_settings(&json, cx);
        });
    }

    fn with_main(cx: &mut App, f: impl FnOnce(&mut AppShell, &mut Window, &mut Context<AppShell>)) {
        let handle: Option<WindowHandle<AppShell>> = cx
            .windows()
            .into_iter()
            .find_map(|w| w.downcast::<AppShell>());
        if let Some(handle) = handle {
            handle
                .update(cx, |shell, window, cx| f(shell, window, cx))
                .ok();
        }
    }

    fn apply_and_refresh_gutter(cx: &mut App, prefs: PandaPrefs) {
        Self::persist_prefs(cx, prefs);
        Self::with_main(cx, |shell, _window, cx| {
            shell.apply_gutter_settings(cx);
        });
    }

    fn toggle_vim(&mut self, state: &ToggleState, cx: &mut Context<Self>) {
        let (ToggleState::Selected | ToggleState::Unselected) = state else {
            return;
        };
        let mut prefs = Self::current_prefs(cx);
        prefs.vim = matches!(state, ToggleState::Selected);
        Self::persist_prefs(cx, prefs);
        cx.notify();
    }

    fn toggle_line_numbers(&mut self, state: &ToggleState, cx: &mut Context<Self>) {
        let (ToggleState::Selected | ToggleState::Unselected) = state else {
            return;
        };
        let mut prefs = Self::current_prefs(cx);
        prefs.line_numbers = matches!(state, ToggleState::Selected);
        Self::apply_and_refresh_gutter(cx, prefs);
        cx.notify();
    }

    fn toggle_breakpoints(&mut self, state: &ToggleState, cx: &mut Context<Self>) {
        let (ToggleState::Selected | ToggleState::Unselected) = state else {
            return;
        };
        let mut prefs = Self::current_prefs(cx);
        prefs.breakpoints = matches!(state, ToggleState::Selected);
        Self::apply_and_refresh_gutter(cx, prefs);
        cx.notify();
    }

    fn select_theme(&mut self, theme: &'static str, cx: &mut Context<Self>) {
        let mut prefs = Self::current_prefs(cx);
        if prefs.theme == theme {
            return;
        }
        prefs.theme = theme.into();
        Self::persist_prefs(cx, prefs);
        // theme_settings observes SettingsStore and refreshes every Panda
        // window, including the editor syntax theme, without recreating it.
        cx.notify();
    }
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = Self::current_prefs(cx);
        let data_dir = app_data_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "(unknown)".into());

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
                    .child(Label::new("Settings").size(LabelSize::Large))
                    .child(
                        Label::new("Panda Note v0.1.0")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(Divider::horizontal())
                    .child(Label::new("Appearance").size(LabelSize::Default))
                    .child(
                        Label::new("Theme")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(DropdownMenu::new(
                        "settings-theme",
                        prefs.theme.clone(),
                        theme_menu(_window, cx),
                    ))
                    .child(Divider::horizontal())
                    .child(SwitchField::new(
                        "toggle-vim",
                        Some("Vim mode"),
                        Some(
                            "Enable modal editing. Restart Panda Note after changing this.".into(),
                        ),
                        if prefs.vim {
                            ToggleState::Selected
                        } else {
                            ToggleState::Unselected
                        },
                        cx.listener(|this, state, _window, cx| {
                            this.toggle_vim(state, cx);
                        }),
                    ))
                    .child(Divider::horizontal())
                    .child(Label::new("Editor Gutter").size(LabelSize::Default))
                    .child(SwitchField::new(
                        "toggle-line-numbers",
                        Some("Show line numbers"),
                        Some("Show line numbers in the editor gutter.".into()),
                        if prefs.line_numbers {
                            ToggleState::Selected
                        } else {
                            ToggleState::Unselected
                        },
                        cx.listener(|this, state, _window, cx| {
                            this.toggle_line_numbers(state, cx);
                        }),
                    ))
                    .child(SwitchField::new(
                        "toggle-breakpoints",
                        Some("Show breakpoints"),
                        Some("Show breakpoints in the gutter.".into()),
                        if prefs.breakpoints {
                            ToggleState::Selected
                        } else {
                            ToggleState::Unselected
                        },
                        cx.listener(|this, state, _window, cx| {
                            this.toggle_breakpoints(state, cx);
                        }),
                    ))
                    .child(Divider::horizontal())
                    .child(
                        Label::new(format!("Data directory:\n{data_dir}"))
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    ),
            )
            .map(|root| client_side_decorations(root, _window, cx))
    }
}

fn theme_menu(window: &mut Window, cx: &mut Context<SettingsWindow>) -> Entity<ContextMenu> {
    const THEMES: &[&str] = &[
        "One Dark",
        "One Light",
        "Ayu Dark",
        "Ayu Light",
        "Gruvbox Dark",
        "Gruvbox Dark Hard",
        "Gruvbox Dark Soft",
        "Gruvbox Light",
        "Gruvbox Light Hard",
        "Gruvbox Light Soft",
    ];
    let entity = cx.weak_entity();
    ContextMenu::build(window, cx, move |menu, _, _| {
        THEMES.iter().fold(menu, |menu, theme| {
            let entity = entity.clone();
            let theme = *theme;
            menu.entry(theme, None, move |_window, cx| {
                entity
                    .update(cx, |this, cx| this.select_theme(theme, cx))
                    .ok();
            })
        })
    })
}
