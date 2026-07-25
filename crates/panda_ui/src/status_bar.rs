//! Bottom status bar for AppShell.

use editor::{Editor, ToPoint};
use gpui::{App, Context, Entity, Window, prelude::*};
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{ContextMenu, PopoverMenu, Tooltip};
use vim::ModeIndicator;

use crate::shell::AppShell;
use crate::state::Mode;
use crate::widgets::sync_indicator;

impl AppShell {
    pub(crate) fn create_vim_indicator(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ModeIndicator> {
        cx.new(|cx| ModeIndicator::new(window, cx))
    }

    pub(crate) fn render_status_bar(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.status_bar_visible {
            return None;
        }
        let Some(vim) = self.vim_indicator.clone() else {
            return None;
        };

        let (instance_name, instance_id, nav_collapsed, sync, hint) = match &self.mode {
            Mode::Setup(_) => (
                "No instance".to_string(),
                None,
                false,
                None,
                None::<SharedString>,
            ),
            Mode::Main(main) => (
                main.instance.name.clone(),
                Some(main.instance.id.clone()),
                main.nav_collapsed,
                main.active.as_ref().map(|a| a.sync_state),
                {
                    let s = main.status.as_ref();
                    if s == "Synced" || s == "LocalModified" || s.starts_with(&main.instance.name) {
                        None
                    } else {
                        Some(main.status.clone())
                    }
                },
            ),
        };

        let cursor = match &self.mode {
            Mode::Main(main) => main
                .editor
                .as_ref()
                .map(|e| cursor_label(e, cx))
                .unwrap_or_else(|| "—".into()),
            _ => "—".into(),
        };

        let nav_icon = if nav_collapsed {
            IconName::ThreadsSidebarLeftClosed
        } else {
            IconName::ThreadsSidebarLeftOpen
        };

        let instances: Vec<_> = self.session.instances().to_vec();
        let shell = cx.weak_entity();
        let active_id = instance_id.clone();

        let instance_switcher = PopoverMenu::new("status-instance-switcher")
            .trigger(
                Button::new("status-instance-trigger", instance_name.clone())
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .start_icon(
                        Icon::new(IconName::Server)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    ),
            )
            .menu(move |window, cx| {
                let shell = shell.clone();
                let active_id = active_id.clone();
                let instances = instances.clone();
                ContextMenu::build(window, cx, move |mut menu, _, _| {
                    if instances.is_empty() {
                        return menu.label("No instances");
                    }
                    for inst in instances {
                        let id = inst.id.clone();
                        let name = if active_id.as_deref() == Some(id.as_str()) {
                            format!("✓ {name}", name = inst.name)
                        } else {
                            inst.name.clone()
                        };
                        let shell = shell.clone();
                        menu = menu.entry(name, None, move |window, cx| {
                            shell
                                .update(cx, |this, cx| {
                                    this.switch_instance(&id, window, cx);
                                })
                                .ok();
                        });
                    }
                    menu
                })
                .into()
            });

        Some(
            h_flex()
                .id("panda-status-bar")
                .w_full()
                .h(px(28.))
                .px_2()
                .gap_2()
                .border_t_1()
                .border_color(cx.theme().colors().border)
                .bg(cx.theme().colors().status_bar_background)
                .justify_between()
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            IconButton::new("status-toggle-nav", nav_icon)
                                .icon_size(IconSize::Small)
                                .style(ButtonStyle::Subtle)
                                .tooltip(Tooltip::text("Toggle navigation"))
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.toggle_nav_pane(cx);
                                })),
                        )
                        .child(instance_switcher)
                        .children(sync.map(sync_indicator))
                        .children(
                            hint.map(|h| Label::new(h).size(LabelSize::XSmall).color(Color::Muted)),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(vim)
                        .child(
                            Label::new("Markdown")
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                        )
                        .child(
                            Label::new(cursor)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                        ),
                )
                .into_any_element(),
        )
    }
}

fn cursor_label(editor: &Entity<Editor>, cx: &App) -> String {
    let editor = editor.read(cx);
    let anchor = editor.selections.newest_anchor().head();
    let snapshot = editor.buffer().read(cx).snapshot(cx);
    let point = anchor.to_point(&snapshot);
    format!("Ln {}, Col {}", point.row + 1, point.column + 1)
}
