//! Chinese word boundaries for Vim word motions via jieba-rs.
//!
//! Classic Vim treats a run of Han characters as a single `CharKind::Word`.
//! These helpers segment contiguous CJK runs with jieba so `w`/`b`/`e`/`ge`
//! and `iw`/`aw` can stop on dictionary words.

use std::ops::Range;
use std::sync::OnceLock;

use editor::{
    Bias, DisplayPoint, MultiBufferOffset, ToPoint,
    display_map::{DisplaySnapshot, ToDisplayPoint},
};
use jieba_rs::{Jieba, TokenizeMode};
use unicode_script::{Script, UnicodeScript};

static JIEBA: OnceLock<Jieba> = OnceLock::new();

pub(crate) fn jieba() -> &'static Jieba {
    JIEBA.get_or_init(Jieba::new)
}

pub(crate) fn is_cjk_word_char(ch: char) -> bool {
    matches!(ch.script(), Script::Han | Script::Tangut | Script::Yi)
}

#[derive(Debug, Clone)]
struct CjkRun {
    start: MultiBufferOffset,
    text: String,
}

#[derive(Debug, Clone, Copy)]
struct TokenSpan {
    /// Byte offset from the start of the CJK run.
    start: usize,
    /// Exclusive byte offset from the start of the CJK run.
    end: usize,
}

impl CjkRun {
    fn absolute(&self, relative: usize) -> MultiBufferOffset {
        self.start + relative
    }

    fn tokens(&self) -> Vec<TokenSpan> {
        tokenize_cjk(&self.text)
    }
}

fn tokenize_cjk(text: &str) -> Vec<TokenSpan> {
    let mut byte_offset = 0;
    jieba()
        .tokenize(text, TokenizeMode::Default, true)
        .into_iter()
        .map(|token| {
            let start = byte_offset;
            let end = byte_offset + token.word.len();
            byte_offset = end;
            TokenSpan { start, end }
        })
        .collect()
}

fn cjk_run_containing(map: &DisplaySnapshot, offset: MultiBufferOffset) -> Option<CjkRun> {
    let on_cjk = map
        .buffer_chars_at(offset)
        .next()
        .is_some_and(|(ch, _)| is_cjk_word_char(ch));
    if !on_cjk {
        return None;
    }

    let mut run_start = offset;
    for (ch, char_offset) in map.reverse_buffer_chars_at(offset) {
        if is_cjk_word_char(ch) {
            run_start = char_offset;
        } else {
            break;
        }
    }

    let mut run_end = offset;
    for (ch, char_offset) in map.buffer_chars_at(offset) {
        if is_cjk_word_char(ch) {
            run_end = char_offset + ch.len_utf8();
        } else {
            break;
        }
    }

    let text = map
        .buffer_snapshot()
        .text_for_range(run_start..run_end)
        .collect::<String>();
    if text.is_empty() {
        return None;
    }
    Some(CjkRun {
        start: run_start,
        text,
    })
}

fn last_char_offset(map: &DisplaySnapshot, exclusive_end: MultiBufferOffset) -> MultiBufferOffset {
    map.reverse_buffer_chars_at(exclusive_end)
        .next()
        .map(|(_, offset)| offset)
        .unwrap_or(exclusive_end)
}

fn offset_to_display(map: &DisplaySnapshot, offset: MultiBufferOffset) -> DisplayPoint {
    map.clip_point(offset.to_display_point(map), Bias::Left)
}

fn exclusive_end_to_display(map: &DisplaySnapshot, offset: MultiBufferOffset) -> DisplayPoint {
    let point = offset.to_point(map.buffer_snapshot());
    map.clip_point(map.point_to_display_point(point, Bias::Right), Bias::Right)
}

/// Next jieba token start strictly after `point`, if still inside the same CJK run.
pub(crate) fn next_word_start(map: &DisplaySnapshot, point: DisplayPoint) -> Option<DisplayPoint> {
    let offset = point.to_offset(map, Bias::Left);
    let run = cjk_run_containing(map, offset)?;
    let relative = offset - run.start;
    let next_start = run
        .tokens()
        .into_iter()
        .map(|token| token.start)
        .find(|&start| start > relative)?;
    Some(offset_to_display(map, run.absolute(next_start)))
}

/// Previous/current jieba token start: last token start strictly before `point`.
pub(crate) fn previous_word_start(
    map: &DisplaySnapshot,
    point: DisplayPoint,
) -> Option<DisplayPoint> {
    let offset = point.to_offset(map, Bias::Left);
    let run = cjk_run_containing(map, offset)?;
    let relative = offset - run.start;
    let prev_start = run
        .tokens()
        .into_iter()
        .map(|token| token.start)
        .filter(|&start| start < relative)
        .next_back()?;
    Some(offset_to_display(map, run.absolute(prev_start)))
}

/// End of the next jieba token that ends after `point` (lands on last character).
pub(crate) fn next_word_end(map: &DisplaySnapshot, point: DisplayPoint) -> Option<DisplayPoint> {
    let offset = point.to_offset(map, Bias::Left);
    let run = cjk_run_containing(map, offset)?;
    let relative = offset - run.start;
    for token in run.tokens() {
        let last = last_char_offset(map, run.absolute(token.end));
        let last_relative = last - run.start;
        if last_relative > relative {
            return Some(offset_to_display(map, last));
        }
    }
    None
}

/// End of the previous jieba token (lands on last character of that token).
pub(crate) fn previous_word_end(
    map: &DisplaySnapshot,
    point: DisplayPoint,
) -> Option<DisplayPoint> {
    let offset = point.to_offset(map, Bias::Left);
    let run = cjk_run_containing(map, offset)?;
    let relative = offset - run.start;
    let mut best: Option<MultiBufferOffset> = None;
    for token in run.tokens() {
        let last = last_char_offset(map, run.absolute(token.end));
        let last_relative = last - run.start;
        if last_relative < relative {
            best = Some(last);
        }
    }
    best.map(|offset| offset_to_display(map, offset))
}

/// Range covering the jieba token under the cursor, optionally extended by `times`.
pub(crate) fn word_range(
    map: &DisplaySnapshot,
    point: DisplayPoint,
    times: usize,
) -> Option<Range<DisplayPoint>> {
    let times = times.max(1);
    let offset = point.to_offset(map, Bias::Left);
    let run = cjk_run_containing(map, offset)?;
    let relative = offset - run.start;
    let tokens = run.tokens();
    let idx = tokens
        .iter()
        .position(|token| token.start <= relative && relative < token.end)?;
    let end_idx = (idx + times - 1).min(tokens.len() - 1);
    let start = run.absolute(tokens[idx].start);
    let end = run.absolute(tokens[end_idx].end);
    Some(offset_to_display(map, start)..exclusive_end_to_display(map, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_nihao_shijie() {
        let tokens = tokenize_cjk("你好世界");
        assert_eq!(
            tokens
                .iter()
                .map(|t| &"你好世界"[t.start..t.end])
                .collect::<Vec<_>>(),
            vec!["你好", "世界"]
        );
        assert_eq!(tokens[0].start, 0);
        assert_eq!(tokens[0].end, "你好".len());
        assert_eq!(tokens[1].start, "你好".len());
        assert_eq!(tokens[1].end, "你好世界".len());
    }

    #[test]
    fn jieba_singleton_lazy_loads() {
        let a = jieba() as *const Jieba;
        let b = jieba() as *const Jieba;
        assert_eq!(a, b);
    }
}
