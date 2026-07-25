(emphasis) @emphasis.markup

(strong_emphasis) @emphasis.strong.markup

(code_span) @text.literal.markup

(strikethrough) @strikethrough.markup

; Dim *, _, `, ~~ delimiters (keep markers visible)
[
  (emphasis_delimiter)
  (code_span_delimiter)
] @punctuation.markup

[
  (inline_link)
  (shortcut_link)
  (collapsed_reference_link)
  (full_reference_link)
  (image)
  (link_text)
  (link_label)
] @link_text.markup

; Link / image punctuation vs destination
(inline_link
  [
    "["
    "]"
    "("
    ")"
  ] @punctuation.markup)

(image
  [
    "!"
    "["
    "]"
    "("
    ")"
  ] @punctuation.markup)

(shortcut_link
  [
    "["
    "]"
  ] @punctuation.markup)

(collapsed_reference_link
  [
    "["
    "]"
  ] @punctuation.markup)

(full_reference_link
  [
    "["
    "]"
  ] @punctuation.markup)

[
  (link_destination)
  (uri_autolink)
  (email_autolink)
] @link_uri.markup
