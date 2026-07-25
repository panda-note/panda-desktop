use gpui::{AnyElement, Context, SharedString, Window, div, prelude::*};
use theme::ActiveTheme;
use ui::ListItem;
use ui::prelude::*;

use crate::shell::AppShell;
use crate::state::Mode;
use crate::widgets::field_row;

impl AppShell {
    pub(crate) fn render_setup(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Mode::Setup(setup) = &self.mode else {
            return div().into_any_element();
        };
        let busy = setup.busy;
        let error = setup.error.clone();
        let name = setup.name.clone();
        let api_url = setup.api_url.clone();
        let username = setup.username.clone();
        let password = setup.password.clone();
        let token = setup.token.clone();
        let saved: Vec<_> = self
            .session
            .instances()
            .iter()
            .map(|i| (i.id.clone(), i.name.clone(), i.api_url.clone()))
            .collect();

        let mut saved_list = v_flex().w(px(420.)).gap_1();
        for (id, inst_name, url) in saved {
            let id_click = id.clone();
            saved_list = saved_list.child(
                ListItem::new(SharedString::from(format!("setup-{id}")))
                    .inset(true)
                    .start_slot(
                        Icon::new(IconName::Server)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        v_flex()
                            .child(Label::new(inst_name).size(LabelSize::Small))
                            .child(Label::new(url).size(LabelSize::XSmall).color(Color::Muted)),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let _ = this.session.set_active(&id_click);
                        if let Some(instance) = this.session.active().cloned() {
                            this.enter_main(instance, window, cx);
                        }
                    })),
            );
        }

        let mut root = v_flex()
            .size_full()
            .bg(cx.theme().colors().background)
            .text_color(cx.theme().colors().text)
            .items_center()
            .justify_center()
            .gap_3()
            .child(Label::new("Panda Note").size(LabelSize::Large))
            .child(
                Button::new("open-instances-window", "Manage Instances…")
                    .style(ButtonStyle::Subtle)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_instances_window(window, cx)),
                    ),
            );

        if !self.session.instances().is_empty() {
            root = root
                .child(Label::new("Saved instances").size(LabelSize::Small))
                .child(saved_list);
        }

        root = root
            .child(Label::new("Add server instance").size(LabelSize::Small))
            .child(field_row("Name", name, cx))
            .child(field_row("API URL", api_url, cx))
            .child(field_row("Username", username, cx))
            .child(field_row("Password", password, cx))
            .child(
                Button::new(
                    "connect-login",
                    if busy {
                        "Connecting..."
                    } else {
                        "Login and Connect"
                    },
                )
                .style(ButtonStyle::Filled)
                .disabled(busy)
                .on_click(cx.listener(|this, _, window, cx| this.connect_login(window, cx))),
            )
            .child(Label::new("or paste API token").size(LabelSize::XSmall))
            .child(field_row("Token", token, cx))
            .child(
                Button::new("connect-token", "Connect with Token")
                    .style(ButtonStyle::Subtle)
                    .on_click(cx.listener(|this, _, window, cx| this.connect_token(window, cx))),
            );

        if let Some(err) = error {
            root = root.child(Label::new(err).size(LabelSize::Small).color(Color::Error));
        }
        root.into_any_element()
    }
}
