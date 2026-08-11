//! Lightweight Settings window (not Zed settings_ui).

use std::sync::Arc;

use editor::{Editor, EditorSettings};
use gpui::{
    AnyElement, App, Bounds, Context, Entity, FocusHandle, Focusable, TitlebarOptions,
    UpdateGlobal, Window, WindowBounds, WindowHandle, WindowOptions, point, prelude::*, px, size,
};
use platform_title_bar::PlatformTitleBar;
use settings::{Settings, SettingsStore};
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{ContextMenu, Divider, DropdownMenu, SwitchField, ToggleState};
use vim_mode_setting::VimModeSetting;
use workspace::client_side_decorations;

use panda_session::app_data_dir;
use panda_store::LocalStore;

use crate::shell::AppShell;

#[derive(Clone)]
struct PandaPrefs {
    vim: bool,
    line_numbers: bool,
    breakpoints: bool,
    theme: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    General,
    Appearance,
    Editor,
    Advanced,
}

pub struct SettingsWindow {
    title_bar: Option<Entity<PlatformTitleBar>>,
    focus_handle: FocusHandle,
    store: Arc<LocalStore>,
    journal_folder: Entity<Editor>,
    section: SettingsSection,
}

impl SettingsWindow {
    pub fn open(store: Arc<LocalStore>, cx: &mut App) {
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
                        size(px(760.), px(600.)),
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
                |window, cx| cx.new(|cx| SettingsWindow::new(store, window, cx)),
            );
        });
    }

    fn new(store: Arc<LocalStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
        let journal_folder = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_text(
                store
                    .get_meta("journal_folder")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "Journal".into()),
                window,
                cx,
            );
            editor.set_show_gutter(false, cx);
            editor.set_placeholder_text("Journal", window, cx);
            editor
        });
        Self {
            title_bar,
            focus_handle: cx.focus_handle(),
            store,
            journal_folder,
            section: SettingsSection::General,
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

    fn select_section(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        self.section = section;
        cx.notify();
    }

    fn save_journal_folder(&mut self, cx: &mut Context<Self>) {
        let path = self.journal_folder.read(cx).text(cx).trim().to_string();
        if !path.is_empty() {
            let _ = self.store.set_meta("journal_folder", &path);
        }
        cx.notify();
    }

    fn render_section(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let prefs = Self::current_prefs(cx);
        match self.section {
            SettingsSection::General => v_flex()
                .gap_4()
                .child(Label::new("General").size(LabelSize::Large))
                .child(
                    Label::new("Journal")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    Label::new("Journal folder")
                        .size(LabelSize::Default),
                )
                .child(
                    Label::new("The Create Journal command makes YYYY/MM folders below this location and opens a YYYYMMDD note.")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    div()
                        .h(px(34.))
                        .w_full()
                        .border_1()
                        .border_color(cx.theme().colors().border)
                        .rounded_md()
                        .px_2()
                        .flex()
                        .items_center()
                        .child(self.journal_folder.clone()),
                )
                .child(
                    Button::new("save-journal-folder", "Save Journal Location")
                        .on_click(cx.listener(|this, _, _, cx| this.save_journal_folder(cx))),
                )
                .into_any_element(),
            SettingsSection::Appearance => v_flex()
                .gap_4()
                .child(Label::new("Appearance").size(LabelSize::Large))
                .child(
                    Label::new("Theme")
                        .size(LabelSize::Default),
                )
                .child(
                    Label::new("Choose the color theme used throughout Panda Note.")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(DropdownMenu::new(
                    "settings-theme",
                    prefs.theme.clone(),
                    theme_menu(window, cx),
                ))
                .into_any_element(),
            SettingsSection::Editor => v_flex()
                .gap_3()
                .child(Label::new("Editor").size(LabelSize::Large))
                .child(SwitchField::new(
                    "toggle-vim",
                    Some("Vim mode"),
                    Some("Enable modal editing. Restart Panda Note after changing this.".into()),
                    if prefs.vim { ToggleState::Selected } else { ToggleState::Unselected },
                    cx.listener(|this, state, _window, cx| this.toggle_vim(state, cx)),
                ))
                .child(Divider::horizontal())
                .child(SwitchField::new(
                    "toggle-line-numbers",
                    Some("Show line numbers"),
                    Some("Show line numbers in the editor gutter.".into()),
                    if prefs.line_numbers { ToggleState::Selected } else { ToggleState::Unselected },
                    cx.listener(|this, state, _window, cx| this.toggle_line_numbers(state, cx)),
                ))
                .child(SwitchField::new(
                    "toggle-breakpoints",
                    Some("Show breakpoints"),
                    Some("Show breakpoints in the gutter.".into()),
                    if prefs.breakpoints { ToggleState::Selected } else { ToggleState::Unselected },
                    cx.listener(|this, state, _window, cx| this.toggle_breakpoints(state, cx)),
                ))
                .into_any_element(),
            SettingsSection::Advanced => v_flex()
                .gap_4()
                .child(Label::new("Advanced").size(LabelSize::Large))
                .child(Label::new("Storage").size(LabelSize::Default))
                .child(
                    Label::new(format!(
                        "Data directory:\n{}",
                        app_data_dir()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|_| "(unknown)".into())
                    ))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
                )
                .into_any_element(),
        }
    }
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let section = self.section;
        let navigation = [
            (SettingsSection::General, "General", "settings-general"),
            (
                SettingsSection::Appearance,
                "Appearance",
                "settings-appearance",
            ),
            (SettingsSection::Editor, "Editor", "settings-editor"),
            (SettingsSection::Advanced, "Advanced", "settings-advanced"),
        ];
        v_flex()
            .size_full()
            .text_color(cx.theme().colors().text)
            .children(self.title_bar.clone())
            .child(
                h_flex()
                    .flex_1()
                    .bg(cx.theme().colors().editor_background)
                    .child(
                        v_flex()
                            .w(px(180.))
                            .h_full()
                            .p_3()
                            .gap_1()
                            .border_r_1()
                            .border_color(cx.theme().colors().border)
                            .bg(cx.theme().colors().panel_background)
                            .child(Label::new("Settings").size(LabelSize::Default).mb_2())
                            .children(navigation.into_iter().map(|(item, label, id)| {
                                div()
                                    .id(id)
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .when(section == item, |this| {
                                        this.bg(cx.theme().colors().ghost_element_selected)
                                    })
                                    .hover(|this| this.bg(cx.theme().colors().ghost_element_hover))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_section(item, cx);
                                    }))
                                    .child(Label::new(label).size(LabelSize::Small))
                            })),
                    )
                    .child(
                        v_flex()
                            .id("settings-content")
                            .flex_1()
                            .h_full()
                            .overflow_y_scroll()
                            .p_6()
                            .max_w(px(760.))
                            .child(self.render_section(window, cx)),
                    ),
            )
            .map(|root| client_side_decorations(root, window, cx))
    }
}

fn theme_menu(window: &mut Window, cx: &mut Context<SettingsWindow>) -> Entity<ContextMenu> {
    const THEMES: &[&str] = &[
        "One Dark",
        "One Light",
        "Ayu Dark",
        "Ayu Light",
        "Ayu Mirage",
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
