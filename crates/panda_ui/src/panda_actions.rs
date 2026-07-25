//! Panda UI / menu actions (no Workspace).

use gpui::actions;

actions!(
    panda,
    [
        /// Open the Instances window.
        OpenInstances,
        /// Open the Settings window.
        OpenSettings,
        /// Open the About window.
        OpenAbout,
        /// Create a new memo in the active notebook.
        NewMemo,
        /// Delete the selected memo.
        DeleteMemo,
        /// Refresh notebooks and memos from the server.
        Refresh,
        /// Toggle markdown preview beside the editor.
        TogglePreview,
        /// Toggle the bottom status bar.
        ToggleStatusBar,
        /// Toggle the left navigation pane.
        ToggleNavPane,
        /// Save the active memo immediately.
        SaveMemo,
        /// Format the active memo.
        FormatMemo,
        /// Toggle the command palette.
        ToggleCommandPalette,
        /// Go to line in the active editor.
        GoToLine,
        /// Quit the application.
        Quit,
    ]
);

pub fn app_menus() -> Vec<gpui::Menu> {
    use editor::actions::{Copy, Cut, Paste, Redo, Undo};
    use gpui::{Menu, MenuItem, OsAction};

    vec![
        Menu {
            name: "File".into(),
            disabled: false,
            items: vec![
                MenuItem::action("New Memo", NewMemo),
                MenuItem::action("Save Memo", SaveMemo),
                MenuItem::action("Delete Memo", DeleteMemo),
                MenuItem::action("Refresh", Refresh),
                MenuItem::separator(),
                MenuItem::action("Instances…", OpenInstances),
                MenuItem::action("Settings…", OpenSettings),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ],
        },
        Menu {
            name: "Edit".into(),
            disabled: false,
            items: vec![
                MenuItem::os_action("Undo", Undo, OsAction::Undo),
                MenuItem::os_action("Redo", Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::separator(),
                MenuItem::action("Format Document", FormatMemo),
                MenuItem::action("Go to Line…", GoToLine),
            ],
        },
        Menu {
            name: "View".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Command Palette…", ToggleCommandPalette),
                MenuItem::action("Toggle Navigation", ToggleNavPane),
                MenuItem::action("Toggle Markdown Preview", TogglePreview),
                MenuItem::action("Toggle Status Bar", ToggleStatusBar),
            ],
        },
        Menu {
            name: "Help".into(),
            disabled: false,
            items: vec![MenuItem::action("About Panda Note", OpenAbout)],
        },
    ]
}
