//! Client-side File/Edit/View/Help menu strip (Zed ApplicationMenu lite).

use gpui::{
    App, Context, Entity, FocusHandle, MouseButton, OwnedMenu, OwnedMenuItem, Render, Window, div,
    prelude::*,
};
use smallvec::SmallVec;
use ui::{
    Button, ButtonCommon, ButtonStyle, ContextMenu, LabelSize, PopoverMenu, PopoverMenuHandle,
    prelude::*,
};

#[derive(Clone)]
struct MenuEntry {
    menu: OwnedMenu,
    handle: PopoverMenuHandle<ContextMenu>,
}

pub struct AppMenuBar {
    entries: SmallVec<[MenuEntry; 8]>,
    action_context: Option<FocusHandle>,
}

impl AppMenuBar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let menus = cx.get_menus().unwrap_or_default();
        Self {
            entries: menus
                .into_iter()
                .map(|menu| MenuEntry {
                    menu,
                    handle: PopoverMenuHandle::default(),
                })
                .collect(),
            action_context: None,
        }
    }

    pub fn set_action_context(&mut self, handle: FocusHandle) {
        self.action_context = Some(handle);
    }

    fn build_menu(
        entry: MenuEntry,
        action_context: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<ContextMenu> {
        ContextMenu::build(window, cx, |menu, _window, _cx| {
            let menu = menu.when_some(action_context, |menu, focused| menu.context(focused));
            entry
                .menu
                .items
                .into_iter()
                .fold(menu, |menu, item| match item {
                    OwnedMenuItem::Separator => menu.separator(),
                    OwnedMenuItem::Action {
                        name,
                        action,
                        checked,
                        disabled,
                        ..
                    } => menu.action_checked_with_disabled(name, action, checked, disabled),
                    OwnedMenuItem::Submenu(submenu) => {
                        submenu
                            .items
                            .into_iter()
                            .fold(menu, |menu, item| match item {
                                OwnedMenuItem::Separator => menu.separator(),
                                OwnedMenuItem::Action {
                                    name,
                                    action,
                                    checked,
                                    disabled,
                                    ..
                                } => menu
                                    .action_checked_with_disabled(name, action, checked, disabled),
                                _ => menu,
                            })
                    }
                    _ => menu,
                })
        })
    }
}

impl Render for AppMenuBar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let action_context = self.action_context.clone();
        h_flex()
            .id("panda-app-menu-bar")
            // The whole title bar is a Windows drag region. Keep menu clicks
            // inside the menu instead of letting them start a window move.
            .h_full()
            .items_center()
            .gap_0p5()
            .flex_none()
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(self.entries.iter().map(|entry| {
                let handle = entry.handle.clone();
                let menu_name = entry.menu.name.clone();
                let entry = entry.clone();
                let action_context = action_context.clone();
                div()
                    .id(SharedString::from(format!("menu-{menu_name}")))
                    .h_full()
                    .flex_none()
                    .child(
                        PopoverMenu::new(SharedString::from(format!("popover-{menu_name}")))
                            .menu(move |window, cx| {
                                Self::build_menu(entry.clone(), action_context.clone(), window, cx)
                                    .into()
                            })
                            .trigger(
                                Button::new(
                                    SharedString::from(format!("trigger-{menu_name}")),
                                    menu_name.clone(),
                                )
                                .style(ButtonStyle::Subtle)
                                .label_size(LabelSize::Small),
                            )
                            .with_handle(handle),
                    )
            }))
    }
}
