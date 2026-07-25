//! Three-pane Panda Note shell + instance setup screen.

mod about_window;
mod action_handlers;
mod actions;
mod app_menu;
mod command_palette;
mod editor_pane;
mod format_toolbar;
mod instances_window;
mod memo_card;
mod memo_list;
mod nav_pane;
mod panda_actions;
mod settings_window;
mod setup;
mod shell;
mod state;
mod status_bar;
mod title_bar;
mod widgets;

pub use action_handlers::init;
pub use panda_actions::{Quit, app_menus};
pub use shell::AppShell;
