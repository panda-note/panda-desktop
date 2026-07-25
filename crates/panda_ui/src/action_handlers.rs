//! Register app-level action handlers for the main Panda window.

use editor::actions::ToggleGoToLine;
use gpui::{App, WindowHandle};
use workspace::Save as WorkspaceSave;
use zed_actions::command_palette::Toggle as CommandPaletteToggle;

use crate::panda_actions::{
    DeleteMemo, FormatMemo, GoToLine, NewMemo, OpenAbout, OpenInstances, OpenSettings, Quit,
    Refresh, SaveMemo, ToggleCommandPalette, ToggleNavPane, TogglePreview, ToggleStatusBar,
};
use crate::shell::AppShell;

pub fn init(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());

    cx.on_action(|_: &OpenInstances, cx| {
        with_main(cx, |shell, window, cx| {
            shell.open_instances_window(window, cx);
        });
    });

    cx.on_action(|_: &OpenSettings, cx| {
        crate::settings_window::SettingsWindow::open(cx);
    });
    cx.on_action(|_: &zed_actions::OpenSettings, cx| {
        crate::settings_window::SettingsWindow::open(cx);
    });
    cx.on_action(|_: &OpenAbout, cx| {
        crate::about_window::AboutWindow::open(cx);
    });

    cx.on_action(|_: &NewMemo, cx| {
        with_main(cx, |shell, window, cx| shell.create_memo(window, cx));
    });

    cx.on_action(|_: &DeleteMemo, cx| {
        with_main(cx, |shell, window, cx| shell.delete_selected(window, cx));
    });

    cx.on_action(|_: &Refresh, cx| {
        with_main(cx, |shell, window, cx| shell.refresh_remote(window, cx));
    });

    cx.on_action(|_: &SaveMemo, cx| {
        with_main(cx, |shell, _window, cx| shell.save_memo_now(cx));
    });

    cx.on_action(|_: &FormatMemo, cx| {
        with_main(cx, |shell, window, cx| shell.format_memo(window, cx));
    });

    cx.on_action(|_: &WorkspaceSave, cx| {
        with_main(cx, |shell, _window, cx| shell.save_memo_now(cx));
    });

    cx.on_action(|_: &TogglePreview, cx| {
        with_main(cx, |shell, _window, cx| shell.toggle_preview(cx));
    });

    cx.on_action(|_: &ToggleStatusBar, cx| {
        with_main(cx, |shell, _window, cx| shell.toggle_status_bar(cx));
    });

    cx.on_action(|_: &ToggleNavPane, cx| {
        with_main(cx, |shell, _window, cx| shell.toggle_nav_pane(cx));
    });

    cx.on_action(|_: &ToggleCommandPalette, cx| {
        with_main(cx, |shell, window, cx| {
            shell.toggle_command_palette(window, cx);
        });
    });

    // Vim `:` and Ctrl+Shift+P bind to command_palette::Toggle.
    cx.on_action(|_: &CommandPaletteToggle, cx| {
        with_main(cx, |shell, window, cx| {
            shell.toggle_command_palette(window, cx);
        });
    });

    cx.on_action(|_: &GoToLine, cx| {
        with_main(cx, |shell, window, cx| shell.open_go_to_line(window, cx));
    });

    // Ctrl+G → go_to_line::Toggle (ToggleGoToLine).
    cx.on_action(|_: &ToggleGoToLine, cx| {
        with_main(cx, |shell, window, cx| shell.open_go_to_line(window, cx));
    });
}

fn main_shell(cx: &App) -> Option<WindowHandle<AppShell>> {
    cx.windows()
        .into_iter()
        .find_map(|w| w.downcast::<AppShell>())
}

fn with_main(
    cx: &mut App,
    f: impl FnOnce(&mut AppShell, &mut gpui::Window, &mut gpui::Context<AppShell>),
) {
    if let Some(main) = main_shell(cx) {
        main.update(cx, f).ok();
    }
}
