//! Markdown format toolbar for the memo editor.

use editor::actions::{
    InsertHorizontalRule, InsertImage, InsertLink, InsertTable, ToggleBlockQuote, ToggleBold,
    ToggleCodeBlock, ToggleHeading1, ToggleHeading2, ToggleHeading3, ToggleHeading4,
    ToggleHeading5, ToggleHeading6, ToggleInlineCode, ToggleItalic, ToggleOrderedList,
    ToggleStrikethrough, ToggleTaskList, ToggleUnorderedList,
};
use gpui::{Action, AnyElement, Context, FocusHandle, Focusable, Window, div, prelude::*, px};
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{ContextMenu, Divider, DividerColor, DropdownMenu, DropdownStyle, TintColor, Tooltip};

use crate::shell::AppShell;
use crate::state::{MemoDisplayMode, Mode};

impl AppShell {
    pub(crate) fn dispatch_editor_action(
        &mut self,
        action: Box<dyn Action>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &self.mode else {
            return;
        };
        let Some(editor) = main.editor.clone() else {
            return;
        };
        let focus = editor.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.dispatch_action(action, cx);
    }

    pub(crate) fn render_format_toolbar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let Mode::Main(main) = &self.mode else {
            return None;
        };
        let editor = main.editor.as_ref()?;
        let focus = editor.read(cx).focus_handle(cx);
        let display_mode = main.memo_display_mode;

        // Read mode deliberately exposes only projection controls. Formatting a
        // hidden source selection would be surprising, while switching back to
        // Source restores the exact same editor and selection.
        if display_mode == MemoDisplayMode::Read {
            return Some(
                h_flex()
                    .id("memo-format-toolbar")
                    .w_full()
                    .px_2()
                    .py_0p5()
                    .gap_0p5()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().editor_background)
                    .children(display_mode_buttons(display_mode, cx))
                    .into_any_element(),
            );
        }

        let heading_menu = {
            let Mode::Main(main) = &mut self.mode else {
                return None;
            };
            if main.heading_menu.is_none() {
                main.heading_menu = Some(heading_menu(window, cx));
            }
            main.heading_menu.clone().unwrap()
        };

        let toolbar = h_flex()
            .id("memo-format-toolbar")
            .w_full()
            .px_2()
            .py_0p5()
            .gap_0p5()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().editor_background)
            .children(display_mode_buttons(display_mode, cx))
            .child(toolbar_sep())
            .child(
                DropdownMenu::new("fmt-heading", "H", heading_menu)
                    .style(DropdownStyle::Subtle)
                    .trigger_size(ButtonSize::Compact)
                    .trigger_tooltip(Tooltip::text("Heading")),
            )
            .child(toolbar_sep())
            .child(label_fmt_btn(
                "fmt-bold",
                "B",
                "Bold",
                &ToggleBold,
                &focus,
                cx,
            ))
            .child(label_fmt_btn(
                "fmt-italic",
                "I",
                "Italic",
                &ToggleItalic,
                &focus,
                cx,
            ))
            .child(label_fmt_btn(
                "fmt-strike",
                "S",
                "Strikethrough",
                &ToggleStrikethrough,
                &focus,
                cx,
            ))
            .child(label_fmt_btn(
                "fmt-inline-code",
                "`",
                "Inline Code",
                &ToggleInlineCode,
                &focus,
                cx,
            ))
            .child(toolbar_sep())
            .child(icon_fmt_btn(
                "fmt-quote",
                IconName::Quote,
                "Block Quote",
                &ToggleBlockQuote,
                &focus,
                cx,
            ))
            .child(icon_fmt_btn(
                "fmt-ul",
                IconName::ListTree,
                "Unordered List",
                &ToggleUnorderedList,
                &focus,
                cx,
            ))
            .child(icon_fmt_btn(
                "fmt-ol",
                IconName::ListCollapse,
                "Ordered List",
                &ToggleOrderedList,
                &focus,
                cx,
            ))
            .child(icon_fmt_btn(
                "fmt-task",
                IconName::ListTodo,
                "Task List",
                &ToggleTaskList,
                &focus,
                cx,
            ))
            .child(toolbar_sep())
            .child(icon_fmt_btn(
                "fmt-code-block",
                IconName::Code,
                "Code Block",
                &ToggleCodeBlock,
                &focus,
                cx,
            ))
            .child(icon_fmt_btn(
                "fmt-link",
                IconName::Link,
                "Insert Link",
                &InsertLink,
                &focus,
                cx,
            ))
            .child(icon_fmt_btn(
                "fmt-image",
                IconName::Image,
                "Insert Image",
                &InsertImage,
                &focus,
                cx,
            ))
            .child(label_fmt_btn(
                "fmt-table",
                "Tbl",
                "Insert Table",
                &InsertTable,
                &focus,
                cx,
            ))
            .child(icon_fmt_btn(
                "fmt-hr",
                IconName::Dash,
                "Horizontal Rule",
                &InsertHorizontalRule,
                &focus,
                cx,
            ));

        Some(toolbar.into_any_element())
    }
}

fn heading_menu(window: &mut Window, cx: &mut Context<AppShell>) -> gpui::Entity<ContextMenu> {
    let entity = cx.weak_entity();
    ContextMenu::build(window, cx, move |menu, _, _| {
        let h1 = entity.clone();
        let h2 = entity.clone();
        let h3 = entity.clone();
        let h4 = entity.clone();
        let h5 = entity.clone();
        let h6 = entity.clone();
        menu.entry("Heading 1", Some(Box::new(ToggleHeading1)), move |window, cx| {
            h1.update(cx, |this, cx| {
                this.dispatch_editor_action(Box::new(ToggleHeading1), window, cx);
            })
            .ok();
        })
        .entry("Heading 2", Some(Box::new(ToggleHeading2)), move |window, cx| {
            h2.update(cx, |this, cx| {
                this.dispatch_editor_action(Box::new(ToggleHeading2), window, cx);
            })
            .ok();
        })
        .entry("Heading 3", Some(Box::new(ToggleHeading3)), move |window, cx| {
            h3.update(cx, |this, cx| {
                this.dispatch_editor_action(Box::new(ToggleHeading3), window, cx);
            })
            .ok();
        })
        .entry("Heading 4", Some(Box::new(ToggleHeading4)), move |window, cx| {
            h4.update(cx, |this, cx| {
                this.dispatch_editor_action(Box::new(ToggleHeading4), window, cx);
            })
            .ok();
        })
        .entry("Heading 5", Some(Box::new(ToggleHeading5)), move |window, cx| {
            h5.update(cx, |this, cx| {
                this.dispatch_editor_action(Box::new(ToggleHeading5), window, cx);
            })
            .ok();
        })
        .entry("Heading 6", Some(Box::new(ToggleHeading6)), move |window, cx| {
            h6.update(cx, |this, cx| {
                this.dispatch_editor_action(Box::new(ToggleHeading6), window, cx);
            })
            .ok();
        })
    })
}

fn display_mode_buttons(
    selected: MemoDisplayMode,
    cx: &mut Context<AppShell>,
) -> [AnyElement; 3] {
    [
        display_mode_btn(
            "fmt-view-source",
            "Source",
            "Markdown source (Vim/editor)",
            MemoDisplayMode::Source,
            selected,
            cx,
        ),
        display_mode_btn(
            "fmt-view-live",
            "Live",
            "Editor + preview (scroll-follows-editor)",
            MemoDisplayMode::Live,
            selected,
            cx,
        ),
        display_mode_btn(
            "fmt-view-read",
            "Read",
            "Rendered Markdown only",
            MemoDisplayMode::Read,
            selected,
            cx,
        ),
    ]
}

fn display_mode_btn(
    id: &'static str,
    label: &'static str,
    title: &'static str,
    mode: MemoDisplayMode,
    selected: MemoDisplayMode,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    Button::new(id, label)
        .style(if mode == selected {
            ButtonStyle::Tinted(TintColor::Accent)
        } else {
            ButtonStyle::Subtle
        })
        .size(ButtonSize::Compact)
        .label_size(LabelSize::XSmall)
        .tooltip(Tooltip::text(title))
        .on_click(cx.listener(move |this, _, _window, cx| {
            this.set_memo_display_mode(mode, cx);
        }))
        .into_any_element()
}

fn toolbar_sep() -> AnyElement {
    div()
        .mx_1()
        .h(px(16.))
        .child(Divider::vertical().color(DividerColor::Border))
        .into_any_element()
}

fn label_fmt_btn<A: Action + Clone + 'static>(
    id: &'static str,
    label: &'static str,
    title: &'static str,
    action: &A,
    focus: &FocusHandle,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let action_for_tooltip = action.clone();
    let action_for_click = action.clone();
    let focus_for_tooltip = focus.clone();
    Button::new(id, label)
        .style(ButtonStyle::Subtle)
        .size(ButtonSize::Compact)
        .label_size(LabelSize::Small)
        .tooltip(move |_, cx| {
            Tooltip::for_action_in(title, &action_for_tooltip, &focus_for_tooltip, cx)
        })
        .on_click(cx.listener(move |this, _, window, cx| {
            this.dispatch_editor_action(Box::new(action_for_click.clone()), window, cx);
        }))
        .into_any_element()
}

fn icon_fmt_btn<A: Action + Clone + 'static>(
    id: &'static str,
    icon: IconName,
    title: &'static str,
    action: &A,
    focus: &FocusHandle,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let action_for_tooltip = action.clone();
    let action_for_click = action.clone();
    let focus_for_tooltip = focus.clone();
    IconButton::new(id, icon)
        .icon_size(IconSize::Small)
        .style(ButtonStyle::Subtle)
        .tooltip(move |_, cx| {
            Tooltip::for_action_in(title, &action_for_tooltip, &focus_for_tooltip, cx)
        })
        .on_click(cx.listener(move |this, _, window, cx| {
            this.dispatch_editor_action(Box::new(action_for_click.clone()), window, cx);
        }))
        .into_any_element()
}
