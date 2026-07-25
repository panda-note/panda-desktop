use gpui::{
    App, Bounds, Context, Entity, FocusHandle, Focusable, TitlebarOptions, Window, WindowBounds,
    WindowKind, WindowOptions, point, prelude::*, px, size,
};
use platform_title_bar::PlatformTitleBar;
use theme::ActiveTheme;
use ui::prelude::*;
use workspace::client_side_decorations;

pub struct AboutWindow {
    title_bar: Option<Entity<PlatformTitleBar>>,
    focus_handle: FocusHandle,
}

impl AboutWindow {
    pub fn open(cx: &mut App) {
        if let Some(existing) = cx.windows().into_iter().find_map(|w| w.downcast::<Self>()) {
            let _ = existing.update(cx, |_, window, _| window.activate_window());
            return;
        }
        cx.defer(|cx| {
            let _ = cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("Panda Note — About".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(12.), px(12.))),
                    }),
                    focus: true,
                    show: true,
                    is_movable: true,
                    kind: WindowKind::Normal,
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(420.), px(260.)),
                        cx,
                    ))),
                    window_background: cx.theme().window_background_appearance(),
                    window_decorations: Some(gpui::WindowDecorations::Client),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Self::new(window, cx)),
            );
        });
    }

    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title_bar = Some(cx.new(|cx| PlatformTitleBar::new("about-title-bar", cx)));
        if let Some(bar) = &title_bar {
            bar.update(cx, |bar, _| {
                bar.set_children(vec![
                    h_flex()
                        .pl_2()
                        .child(Label::new("About").size(LabelSize::Small))
                        .into_any_element(),
                ]);
            });
        }
        Self {
            title_bar,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for AboutWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AboutWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        client_side_decorations(
            v_flex()
                .size_full()
                .text_color(cx.theme().colors().text)
                .children(self.title_bar.clone())
                .child(
                    v_flex()
                        .flex_1()
                        .gap_2()
                        .p_5()
                        .bg(cx.theme().colors().editor_background)
                        .child(Label::new("Panda Note").size(LabelSize::Large))
                        .child(Label::new("Desktop client built on Zed").color(Color::Muted))
                        .child(Label::new("Version: v0.1.0").size(LabelSize::Small))
                        .child(
                            Label::new("Copyright (c) Panda Note")
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                        ),
                ),
            window,
            cx,
        )
    }
}
