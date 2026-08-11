//! Panda Note desktop entry.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use assets::Assets;
use fs::{Fs, RealFs};
use gpui::{
    App, Bounds, Focusable, TitlebarOptions, UpdateGlobal, WindowBounds, WindowOptions, point,
    prelude::*, px, size,
};
use language::LanguageRegistry;
use node_runtime::NodeRuntime;
use panda_ui::{AppShell, app_menus, init as init_panda_ui};
use settings::{
    BaseKeymap, DEFAULT_KEYMAP_PATH, KeybindSource, KeymapFile, SPECIFIC_OVERRIDES_KEYMAP_PATH,
    Settings, SettingsStore, VIM_KEYMAP_PATH,
};
use theme::LoadThemes;
use theme::{ActiveTheme, GlobalTheme};
use vim_mode_setting::VimModeSetting;

fn main() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            release_channel::init(semver::Version::new(0, 1, 0), cx);
            settings::init(cx);

            SettingsStore::update_global(cx, |store, cx| {
                let _ = store.set_user_settings(
                    r##"{
                        "vim_mode": true,
                        "base_keymap": "VSCode",
                        "current_line_highlight": "gutter",
                        "gutter": {
                            "line_numbers": true,
                            "breakpoints": false,
                            "runnables": false,
                            "bookmarks": false
                        },
                        "experimental.theme_overrides": {
                            "syntax": {
                                "punctuation.markup": { "color": "#5d636fff" },
                                "punctuation.list_marker": { "color": "#878e98ff" },
                                "punctuation.embedded": { "color": "#5d636fff" },
                                "emphasis": { "font_style": "italic", "color": "#74ade8ff" },
                                "emphasis.strong": { "font_weight": 700, "color": "#bf956aff" },
                                "title": { "font_weight": 700, "color": "#e06c75ff" },
                                "text.literal": {
                                    "color": "#a1c181ff",
                                    "background_color": "#3b404a88"
                                }
                            }
                        }
                    }"##,
                    cx,
                );
            });

            theme_settings::init(LoadThemes::All(Box::new(Assets)), cx);
            Assets
                .load_fonts(cx)
                .expect("failed to load fonts from assets");

            let fs = Arc::new(RealFs::new(None, cx.background_executor().clone()));
            <dyn Fs>::set_global(fs.clone(), cx);

            let languages = Arc::new(LanguageRegistry::new(cx.background_executor().clone()));
            languages::init(languages.clone(), fs, NodeRuntime::unavailable(), cx);
            languages.set_theme(cx.theme().clone());
            cx.observe_global::<GlobalTheme>({
                let languages = languages.clone();
                move |cx| {
                    languages.set_theme(cx.theme().clone());
                }
            })
            .detach();

            editor::init(cx);
            search::init(cx);
            vim::init(cx);
            zed_actions::init();
            platform_title_bar::PlatformTitleBar::init(cx);
            load_keymaps(cx);

            cx.set_menus(app_menus());
            init_panda_ui(cx);

            let languages_for_window = languages.clone();
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1280.), px(800.)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Panda Note".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(9.), px(9.))),
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    let shell =
                        cx.new(|cx| AppShell::new(languages_for_window.clone(), window, cx));
                    window.focus(&shell.focus_handle(cx), cx);
                    shell
                },
            )
            .expect("failed to open Panda window");

            cx.activate(true);
        });
}

fn load_keymaps(cx: &mut App) {
    bind_keymap_allowing_unknown(DEFAULT_KEYMAP_PATH, KeybindSource::Default, cx);

    let base_keymap = *BaseKeymap::get_global(cx);
    if base_keymap != BaseKeymap::None
        && let Some(asset_path) = base_keymap.asset_path()
    {
        bind_keymap_allowing_unknown(asset_path, KeybindSource::Base, cx);
    }

    if VimModeSetting::get_global(cx).0 {
        bind_keymap_allowing_unknown(VIM_KEYMAP_PATH, KeybindSource::Vim, cx);
    }

    bind_keymap_allowing_unknown(SPECIFIC_OVERRIDES_KEYMAP_PATH, KeybindSource::Default, cx);
    bind_keymap_allowing_unknown("keymaps/panda.json", KeybindSource::User, cx);
}

fn bind_keymap_allowing_unknown(asset_path: &str, source: KeybindSource, cx: &mut App) {
    match KeymapFile::load_asset_allow_partial_failure(asset_path, cx) {
        Ok(mut key_bindings) => {
            for key_binding in &mut key_bindings {
                key_binding.set_meta(source.meta());
            }
            cx.bind_keys(key_bindings);
        }
        Err(err) => {
            eprintln!("warning: skipping keymap {asset_path}: {err}");
        }
    }
}
