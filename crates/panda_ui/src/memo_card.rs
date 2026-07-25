use gpui::{AnyElement, App, prelude::*, px};
use panda_core::SyncState;
use theme::ActiveTheme;
use ui::{HighlightedLabel, prelude::*};

use crate::widgets::{compact_tag_chip, sync_indicator};

pub(crate) struct MemoCardData {
    pub title: String,
    pub excerpt: String,
    pub tags: Vec<String>,
    pub notebook_name: Option<String>,
    pub is_pinned: bool,
    /// Only search suggestions set this. Ordinary memo rows keep their existing rendering.
    pub highlight_query: Option<String>,
}

fn match_indices(text: &str, query: Option<&str>) -> Vec<usize> {
    let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) else {
        return Vec::new();
    };
    // Search itself is case-insensitive. Lowercasing preserves byte offsets for the normal
    // Latin/CJK content we index; reject any non-boundary index defensively below.
    let haystack = text.to_lowercase();
    let needle = query.to_lowercase();
    haystack
        .match_indices(&needle)
        .flat_map(|(start, _)| {
            let end = start + needle.len();
            text.get(start..end)
                .filter(|_| text.is_char_boundary(start) && text.is_char_boundary(end))
                .map(|segment| {
                    let mut indices = Vec::new();
                    let mut offset = start;
                    for character in segment.chars() {
                        indices.push(offset);
                        offset += character.len_utf8();
                    }
                    indices
                })
                .unwrap_or_default()
        })
        .collect()
}

fn memo_label(
    text: String,
    query: Option<&str>,
    size: LabelSize,
    color: Option<Color>,
) -> AnyElement {
    let indices = match_indices(&text, query);
    if indices.is_empty() {
        let label = Label::new(text).size(size).truncate();
        match color {
            Some(color) => label.color(color).into_any_element(),
            None => label.into_any_element(),
        }
    } else {
        let label = HighlightedLabel::new(text, indices).size(size).truncate();
        match color {
            Some(color) => label.color(color).into_any_element(),
            None => label.into_any_element(),
        }
    }
}

fn memo_card_content(data: MemoCardData, is_selected: bool, cx: &App) -> AnyElement {
    let MemoCardData {
        title,
        excerpt,
        tags,
        notebook_name,
        is_pinned,
        highlight_query,
    } = data;
    let query = highlight_query.as_deref();
    v_flex()
        .gap_0p5()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .children(notebook_name.map(|notebook| {
            div().w_full().min_w_0().child(
                Label::new(notebook)
                    .size(LabelSize::XSmall)
                    .color(Color::Muted)
                    .truncate(),
            )
        }))
        .child(
            h_flex()
                .gap_1()
                .items_center()
                .min_w_0()
                .overflow_hidden()
                .child(div().flex_1().min_w_0().child(memo_label(
                    title,
                    query,
                    LabelSize::Small,
                    None,
                )))
                .children(is_pinned.then(|| {
                    Icon::new(IconName::Pin)
                        .size(IconSize::XSmall)
                        .color(Color::Accent)
                })),
        )
        .child(div().w_full().min_w_0().overflow_hidden().child(memo_label(
            excerpt,
            query,
            LabelSize::XSmall,
            Some(Color::Muted),
        )))
        .children((!tags.is_empty()).then(|| {
            h_flex()
                .gap_1()
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .children(
                    tags.into_iter()
                        // 窄搜索下拉保留一个可读标签，避免多个 chip 把整行挤没。
                        .take(1)
                        .map(|tag| compact_tag_chip(tag, is_selected, cx)),
                )
                .into_any_element()
        }))
        .into_any_element()
}

/// Memo row with equal padding against the list pane edges (avoids ListItem inset asymmetry).
pub(crate) fn memo_list_item(
    row_id: impl Into<gpui::ElementId>,
    data: MemoCardData,
    sync: SyncState,
    is_selected: bool,
    cx: &App,
) -> gpui::Stateful<Div> {
    // 搜索结果额外显示笔记本名，四行内容不能再沿用普通列表 72px 的三行高度。
    let row_height = if data.notebook_name.is_some() {
        px(92.)
    } else {
        px(72.)
    };
    let content = memo_card_content(data, is_selected, cx);
    div()
        .id(row_id.into())
        .w_full()
        .h(row_height)
        .rounded_md()
        .p_2()
        .overflow_hidden()
        .cursor_pointer()
        .when(is_selected, |this| {
            this.bg(cx.theme().colors().ghost_element_selected)
        })
        .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .gap_2()
                .items_start()
                .child(div().flex_1().min_w_0().child(content))
                .child(sync_indicator(sync)),
        )
}
