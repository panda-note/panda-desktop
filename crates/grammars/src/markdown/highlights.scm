[
  (paragraph)
  (pipe_table)
] @text

; Heading text (markers styled separately below)
(atx_heading
  (inline) @title.markup)

(setext_heading
  (paragraph) @title.markup)

(thematic_break) @punctuation.markup

; Dim ATX / setext markers (#, ===, ---)
[
  (atx_h1_marker)
  (atx_h2_marker)
  (atx_h3_marker)
  (atx_h4_marker)
  (atx_h5_marker)
  (atx_h6_marker)
  (setext_h1_underline)
  (setext_h2_underline)
] @punctuation.markup

[
  (list_marker_plus)
  (list_marker_minus)
  (list_marker_star)
  (list_marker_dot)
  (list_marker_parenthesis)
  (task_list_marker_checked)
  (task_list_marker_unchecked)
] @punctuation.list_marker.markup

[
  (block_quote_marker)
  (block_continuation)
] @punctuation.markup

; Mute quote body (comment is typically low-contrast in themes)
(block_quote) @comment

(pipe_table_header
  "|" @punctuation.markup)

(pipe_table_row
  "|" @punctuation.markup)

(pipe_table_delimiter_row
  "|" @punctuation.markup)

(pipe_table_delimiter_cell
  "-" @punctuation.markup)

; Fenced / indented code: unified background via text.literal
(indented_code_block) @text.literal.markup

(fenced_code_block) @text.literal.markup

[
  (fenced_code_block_delimiter)
  (info_string)
] @punctuation.embedded.markup

(link_reference_definition) @link_text.markup

(link_destination) @link_uri.markup
