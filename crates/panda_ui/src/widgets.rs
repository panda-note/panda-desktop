use std::collections::HashMap;

use editor::Editor;
use gpui::{App, Context, Entity, SharedString, Window, div, prelude::*, px};
use panda_core::{ActiveMemo, Notebook, SyncState};
use theme::ActiveTheme;
use ui::prelude::*;
use ui::{Divider, DividerColor, Indicator};

use crate::shell::AppShell;

pub(crate) fn display_title(active: &ActiveMemo) -> &str {
    active
        .title
        .as_deref()
        .filter(|t| !t.is_empty())
        .unwrap_or("Untitled")
}

/// Human-readable notebook path from root to the memo's notebook (names only).
pub(crate) fn notebook_breadcrumb_path(notebooks: &[Notebook], notebook_id: &str) -> Vec<String> {
    let by_id: HashMap<&str, &Notebook> = notebooks
        .iter()
        .map(|notebook| (notebook.id.as_str(), notebook))
        .collect();
    let mut segments = Vec::new();
    let mut current = Some(notebook_id);
    while let Some(id) = current {
        let Some(notebook) = by_id.get(id) else {
            break;
        };
        segments.push(notebook.name.clone());
        current = notebook.parent_id.as_deref();
    }
    segments.reverse();
    segments
}

pub(crate) fn field_editor(
    window: &mut Window,
    cx: &mut Context<AppShell>,
    text: &str,
) -> Entity<Editor> {
    cx.new(|cx| {
        let mut editor = Editor::single_line(window, cx);
        editor.set_text(text.to_string(), window, cx);
        editor.set_show_gutter(false, cx);
        editor
    })
}

pub(crate) fn field_row(
    label: impl Into<SharedString>,
    editor: Entity<Editor>,
    cx: &App,
) -> impl IntoElement {
    ui::v_flex()
        .w_full()
        .gap_1()
        .child(Label::new(label).size(LabelSize::Small))
        .child(
            div()
                .h(px(32.))
                .w_full()
                .flex()
                .items_center()
                .border_1()
                .border_color(cx.theme().colors().border)
                .rounded_md()
                .px_2()
                .child(div().w_full().h_full().flex().items_center().child(editor)),
        )
}

pub(crate) fn section_header(label: impl Into<SharedString>) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_1p5()
        .child(Label::new(label).size(LabelSize::Small).color(Color::Muted))
        .child(Divider::horizontal().color(DividerColor::BorderFaded))
}

pub(crate) fn sync_indicator(state: SyncState) -> Indicator {
    let color = match state {
        SyncState::Synced => Color::Success,
        SyncState::LocalModified | SyncState::LocalCreated | SyncState::LocalDeleted => {
            Color::Warning
        }
        SyncState::Syncing => Color::Accent,
        SyncState::Failed | SyncState::Conflict => Color::Error,
    };
    Indicator::dot().color(color)
}

pub(crate) fn compact_tag_chip(
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> AnyElement {
    let (bg, text) = if selected {
        (cx.theme().colors().text_accent.opacity(0.16), Color::Accent)
    } else {
        (cx.theme().colors().element_background, Color::Muted)
    };
    h_flex()
        .h(px(18.))
        .max_w(px(128.))
        .min_w_0()
        .px_1p5()
        .rounded_sm()
        .bg(bg)
        .items_center()
        .overflow_hidden()
        .child(
            div().flex_1().min_w_0().child(
                Label::new(label)
                    .size(LabelSize::XSmall)
                    .color(text)
                    .line_height_style(LineHeightStyle::UiLabel)
                    .truncate(),
            ),
        )
        .into_any_element()
}

pub(crate) fn removable_tag_chip(
    label: impl Into<SharedString>,
    on_remove: impl Fn(&mut AppShell, &gpui::ClickEvent, &mut Window, &mut Context<AppShell>) + 'static,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let label = label.into();
    let remove_id = SharedString::from(format!("tag-rm-{label}"));
    h_flex()
        .h(px(20.))
        .pl_1p5()
        .pr_0p5()
        .gap_0p5()
        .rounded_sm()
        .bg(cx.theme().colors().text_accent.opacity(0.14))
        .items_center()
        .child(
            Label::new(label.clone())
                .size(LabelSize::XSmall)
                .color(Color::Accent)
                .line_height_style(LineHeightStyle::UiLabel),
        )
        .child(
            IconButton::new(remove_id, IconName::Close)
                .icon_size(IconSize::XSmall)
                .size(ButtonSize::None)
                .style(ButtonStyle::Transparent)
                .icon_color(Color::Accent)
                .on_click(cx.listener(on_remove)),
        )
        .into_any_element()
}

pub(crate) fn filtered_tag_suggestions(
    available_tags: &[String],
    active_tags: &[String],
    query: &str,
) -> Vec<String> {
    let query = query.trim().to_ascii_lowercase();
    let mut matches = available_tags
        .iter()
        .filter(|tag| {
            !active_tags
                .iter()
                .any(|active| active.eq_ignore_ascii_case(tag))
        })
        .filter(|tag| {
            if query.is_empty() {
                return true;
            }
            fuzzy_contains(&tag.to_ascii_lowercase(), &query)
        })
        .take(6)
        .cloned()
        .collect::<Vec<_>>();
    if matches.is_empty() && !query.is_empty() {
        matches.push(query);
    }
    matches
}

fn fuzzy_contains(text: &str, query: &str) -> bool {
    if text.contains(query) {
        return true;
    }
    let mut q_chars = query.chars();
    let mut current = q_chars.next();
    if current.is_none() {
        return true;
    }
    for ch in text.chars() {
        if Some(ch) == current {
            current = q_chars.next();
            if current.is_none() {
                return true;
            }
        }
    }
    false
}
