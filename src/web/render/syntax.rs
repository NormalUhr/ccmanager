//! Syntax highlighting for code blocks, emitting classed HTML spans.
//!
//! We use `ClassedHTMLGenerator` with `ClassStyle::Spaced` so the theming
//! lives in CSS (see `static/style.css`) rather than being inlined into the
//! HTML. Unknown languages fall back to a plain-text `<pre>`.

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use std::sync::OnceLock;

/// Cached `SyntaxSet` — loading on every render would be wasteful.
fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Highlight `source` as `lang` (e.g. "rust", "python", "bash"), returning an
/// HTML fragment suitable for insertion into the page. Always produces a
/// `<pre class="code"><code class="lang-XYZ">…</code></pre>` block, even for
/// unknown languages (where the content is HTML-escaped only).
pub fn highlight_html(source: &str, lang: &str) -> String {
    let ss = syntax_set();
    // `find_syntax_by_token` handles "rust", "py", "sh", "bash", etc.
    let syntax = ss
        .find_syntax_by_token(lang.trim())
        .or_else(|| ss.find_syntax_by_extension(lang.trim()))
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let class_style = ClassStyle::Spaced;
    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, ss, class_style);
    for line in LinesWithEndings::from(source) {
        // parse_html_for_line_which_includes_newline only fails on pathologically
        // malformed input; treat that as "fall back to plain text" by breaking.
        if generator
            .parse_html_for_line_which_includes_newline(line)
            .is_err()
        {
            return plain_fallback(source, lang);
        }
    }
    let inner = generator.finalize();
    let lang_class = safe_lang_class(lang);
    format!(
        "<pre class=\"code\"><code class=\"syntect {}\">{}</code></pre>",
        lang_class, inner
    )
}

fn plain_fallback(source: &str, lang: &str) -> String {
    let escaped = html_escape(source);
    let lang_class = safe_lang_class(lang);
    format!(
        "<pre class=\"code\"><code class=\"syntect {}\">{}</code></pre>",
        lang_class, escaped
    )
}

fn safe_lang_class(lang: &str) -> String {
    let mut out = String::from("lang-");
    for c in lang.chars().take(32) {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust() {
        let out = highlight_html("fn main() {}", "rust");
        assert!(out.starts_with("<pre class=\"code\">"));
        assert!(out.contains("lang-rust"));
    }

    #[test]
    fn unknown_language_escapes_safely() {
        let out = highlight_html("<x>bad</x>", "not-a-language");
        assert!(out.contains("&lt;x&gt;"), "must escape: {}", out);
    }
}
