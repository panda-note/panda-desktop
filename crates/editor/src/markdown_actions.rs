use std::borrow::Cow;

use super::*;

impl Editor {
    pub fn toggle_markdown_block_quote(
        &mut self,
        _: &ToggleBlockQuote,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_markdown_line_prefix(window, cx, "> ", &["> ", ">"]);
    }

    pub fn toggle_markdown_heading_1(
        &mut self,
        _: &ToggleHeading1,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_markdown_heading(1, window, cx);
    }

    pub fn toggle_markdown_heading_2(
        &mut self,
        _: &ToggleHeading2,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_markdown_heading(2, window, cx);
    }

    pub fn toggle_markdown_heading_3(
        &mut self,
        _: &ToggleHeading3,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_markdown_heading(3, window, cx);
    }

    pub fn toggle_markdown_heading_4(
        &mut self,
        _: &ToggleHeading4,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_markdown_heading(4, window, cx);
    }

    pub fn toggle_markdown_heading_5(
        &mut self,
        _: &ToggleHeading5,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_markdown_heading(5, window, cx);
    }

    pub fn toggle_markdown_heading_6(
        &mut self,
        _: &ToggleHeading6,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_markdown_heading(6, window, cx);
    }

    pub fn toggle_markdown_unordered_list(
        &mut self,
        _: &ToggleUnorderedList,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_markdown_line_prefix(window, cx, "- ", &["- ", "* ", "+ "]);
    }

    pub fn toggle_markdown_ordered_list(
        &mut self,
        _: &ToggleOrderedList,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_in_markdown_language(cx) {
            return;
        }
        self.manipulate_mutable_lines(window, cx, |lines| {
            let ordered = lines.iter().all(|line| {
                let trimmed = line.trim_start();
                ordered_list_prefix(trimmed).is_some()
            });
            for (ix, line) in lines.iter_mut().enumerate() {
                let indent = leading_indent(line);
                let body = line[indent.len()..].to_string();
                let stripped = if let Some(prefix) = ordered_list_prefix(&body) {
                    body[prefix.len()..].to_string()
                } else if let Some(prefix) = unordered_or_task_prefix(&body) {
                    body[prefix.len()..].to_string()
                } else {
                    body
                };
                *line = if ordered {
                    Cow::Owned(format!("{indent}{stripped}"))
                } else {
                    Cow::Owned(format!("{indent}{}. {stripped}", ix + 1))
                };
            }
        });
    }

    pub fn toggle_markdown_task_list(
        &mut self,
        _: &ToggleTaskList,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_in_markdown_language(cx) {
            return;
        }
        self.manipulate_mutable_lines(window, cx, |lines| {
            let all_tasks = lines.iter().all(|line| {
                let body = line.trim_start();
                body.starts_with("- [ ] ")
                    || body.starts_with("- [x] ")
                    || body.starts_with("- [X] ")
            });
            for line in lines.iter_mut() {
                let indent = leading_indent(line);
                let body = line[indent.len()..].to_string();
                let stripped = if let Some(rest) = body
                    .strip_prefix("- [ ] ")
                    .or_else(|| body.strip_prefix("- [x] "))
                    .or_else(|| body.strip_prefix("- [X] "))
                {
                    rest.to_string()
                } else if let Some(prefix) =
                    unordered_or_task_prefix(&body).or_else(|| ordered_list_prefix(&body))
                {
                    body[prefix.len()..].to_string()
                } else {
                    body
                };
                *line = if all_tasks {
                    Cow::Owned(format!("{indent}{stripped}"))
                } else {
                    Cow::Owned(format!("{indent}- [ ] {stripped}"))
                };
            }
        });
    }

    pub fn toggle_markdown_bold(
        &mut self,
        _: &ToggleBold,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.wrap_or_unwrap_markdown_inline("**", "**", window, cx);
    }

    pub fn toggle_markdown_italic(
        &mut self,
        _: &ToggleItalic,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.wrap_or_unwrap_markdown_inline("*", "*", window, cx);
    }

    pub fn toggle_markdown_strikethrough(
        &mut self,
        _: &ToggleStrikethrough,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.wrap_or_unwrap_markdown_inline("~~", "~~", window, cx);
    }

    pub fn toggle_markdown_inline_code(
        &mut self,
        _: &ToggleInlineCode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.wrap_or_unwrap_markdown_inline("`", "`", window, cx);
    }

    pub fn toggle_markdown_code_block(
        &mut self,
        _: &ToggleCodeBlock,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_in_markdown_language(cx) || self.read_only(cx) {
            return;
        }
        self.manipulate_mutable_lines(window, cx, |lines| {
            let fenced = lines.first().is_some_and(|l| l.trim() == "```")
                && lines.last().is_some_and(|l| l.trim() == "```")
                && lines.len() >= 2;
            if fenced {
                lines.remove(0);
                lines.pop();
            } else {
                lines.insert(0, Cow::Borrowed("```"));
                lines.push(Cow::Borrowed("```"));
            }
        });
    }

    pub fn insert_markdown_link(
        &mut self,
        _: &InsertLink,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_in_markdown_language(cx) || self.read_only(cx) {
            return;
        }
        let display_snapshot = self.display_snapshot(cx);
        let selection = self
            .selections
            .newest::<MultiBufferOffset>(&display_snapshot);
        let selected: String = self
            .buffer()
            .read(cx)
            .read(cx)
            .text_for_range(selection.start..selection.end)
            .collect();
        let replacement = if selected.is_empty() {
            "[text](url)".to_string()
        } else {
            format!("[{selected}](url)")
        };
        self.insert(&replacement, window, cx);
    }

    pub fn insert_markdown_image(
        &mut self,
        _: &InsertImage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_in_markdown_language(cx) || self.read_only(cx) {
            return;
        }
        let display_snapshot = self.display_snapshot(cx);
        let selection = self
            .selections
            .newest::<MultiBufferOffset>(&display_snapshot);
        let selected: String = self
            .buffer()
            .read(cx)
            .read(cx)
            .text_for_range(selection.start..selection.end)
            .collect();
        let replacement = if selected.is_empty() {
            "![alt](url)".to_string()
        } else {
            format!("![{selected}](url)")
        };
        self.insert(&replacement, window, cx);
    }

    pub fn insert_markdown_table(
        &mut self,
        _: &InsertTable,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_in_markdown_language(cx) || self.read_only(cx) {
            return;
        }
        self.insert(
            "| Column 1 | Column 2 |\n| --- | --- |\n|  |  |\n",
            window,
            cx,
        );
    }

    pub fn insert_markdown_horizontal_rule(
        &mut self,
        _: &InsertHorizontalRule,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_in_markdown_language(cx) || self.read_only(cx) {
            return;
        }
        self.insert("\n---\n", window, cx);
    }

    fn set_markdown_heading(&mut self, level: u8, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_in_markdown_language(cx) {
            return;
        }
        let prefix = format!("{} ", "#".repeat(level as usize));
        self.manipulate_mutable_lines(window, cx, |lines| {
            let all_same = lines.iter().all(|line| {
                let body = line.trim_start();
                body.starts_with(prefix.as_str())
                    && body
                        .get(prefix.len()..)
                        .is_none_or(|rest| !rest.starts_with('#'))
            });
            for line in lines.iter_mut() {
                let indent = leading_indent(line);
                let body = strip_heading_prefix(&line[indent.len()..]);
                *line = if all_same {
                    Cow::Owned(format!("{indent}{body}"))
                } else {
                    Cow::Owned(format!("{indent}{prefix}{body}"))
                };
            }
        });
    }

    fn toggle_markdown_line_prefix(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        add_prefix: &str,
        match_prefixes: &[&str],
    ) {
        if !self.is_in_markdown_language(cx) {
            return;
        }
        let add_prefix = add_prefix.to_string();
        let match_prefixes: Vec<String> = match_prefixes.iter().map(|s| s.to_string()).collect();
        self.manipulate_mutable_lines(window, cx, |lines| {
            let all_prefixed = lines.iter().all(|line| {
                let body = line.trim_start();
                match_prefixes.iter().any(|p| body.starts_with(p.as_str()))
            });
            for line in lines.iter_mut() {
                let indent = leading_indent(line);
                let body = line[indent.len()..].to_string();
                let stripped = match_prefixes
                    .iter()
                    .find_map(|p| body.strip_prefix(p.as_str()))
                    .unwrap_or(body.as_str())
                    .to_string();
                *line = if all_prefixed {
                    Cow::Owned(format!("{indent}{stripped}"))
                } else if stripped.trim().is_empty() && add_prefix.trim() == ">" {
                    Cow::Owned(format!("{indent}>"))
                } else {
                    Cow::Owned(format!("{indent}{add_prefix}{stripped}"))
                };
            }
        });
    }

    fn wrap_or_unwrap_markdown_inline(
        &mut self,
        open: &str,
        close: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_in_markdown_language(cx) || self.read_only(cx) {
            return;
        }

        let display_snapshot = self.display_snapshot(cx);
        let selection = self
            .selections
            .newest::<MultiBufferOffset>(&display_snapshot);
        let buffer = self.buffer().read(cx).read(cx);
        let selected: String = buffer
            .text_for_range(selection.start..selection.end)
            .collect();

        let (replacement, cursor_offset_in_replacement) = if selected.starts_with(open)
            && selected.ends_with(close)
            && selected.len() >= open.len() + close.len()
        {
            let inner = &selected[open.len()..selected.len() - close.len()];
            (inner.to_string(), inner.len())
        } else if selected.is_empty() {
            (format!("{open}{close}"), open.len())
        } else {
            (
                format!("{open}{selected}{close}"),
                open.len() + selected.len(),
            )
        };
        drop(buffer);

        let start = selection.start;
        self.transact(window, cx, |this, window, cx| {
            this.buffer.update(cx, |buffer, cx| {
                buffer.edit([(start..selection.end, replacement.clone())], None, cx);
            });
            let cursor = MultiBufferOffset(start.0 + cursor_offset_in_replacement);
            this.change_selections(Default::default(), window, cx, |s| {
                s.select_ranges([cursor..cursor]);
            });
        });
    }

    fn is_in_markdown_language(&self, cx: &mut App) -> bool {
        let snapshot = self.buffer.read(cx).snapshot(cx);
        let head = self
            .selections
            .newest::<MultiBufferOffset>(&self.display_snapshot(cx))
            .head();
        match snapshot.language_at(head) {
            None => true,
            Some(language) => language.name() == "Markdown",
        }
    }
}

fn leading_indent(line: &str) -> String {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

fn strip_heading_prefix(body: &str) -> String {
    let trimmed = body.trim_start_matches('#');
    trimmed.strip_prefix(' ').unwrap_or(trimmed).to_string()
}

fn unordered_or_task_prefix(body: &str) -> Option<&str> {
    const PREFIXES: &[&str] = &["- [ ] ", "- [x] ", "- [X] ", "- ", "* ", "+ "];
    PREFIXES.iter().copied().find(|p| body.starts_with(p))
}

fn ordered_list_prefix(body: &str) -> Option<&str> {
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    if i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1] == b' ' {
        Some(&body[..i + 2])
    } else {
        None
    }
}
