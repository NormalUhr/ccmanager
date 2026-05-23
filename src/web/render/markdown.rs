//! Render markdown source to HTML using pulldown-cmark.
//!
//! Code blocks are handed off to [`super::syntax::highlight_html`] for
//! syntect-based syntax highlighting with classed spans. Everything else is
//! standard CommonMark + GitHub table/strikethrough/tasklist extensions.

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};

/// Render markdown source to a safe HTML string.
///
/// The output is intended to be inserted inside a `<div class="md">` container
/// styled by the site stylesheet. Code blocks come back as
/// `<pre class="code"><code class="syntect">…spans…</code></pre>`.
pub fn render(src: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(src, opts);
    let mut out = String::with_capacity(src.len() + 64);
    let mut in_code: Option<String> = None;
    let mut code_buf = String::new();

    // We implement a small state machine over the parser events so we can
    // intercept CodeBlock ranges, accumulate their text, and replace the
    // entire block with a syntect-highlighted snippet.
    let mut to_render: Vec<Event> = Vec::new();

    for ev in parser {
        // Coerce any raw HTML event in the source into plain text so user
        // content can't inject elements. This is the only defense layer —
        // everything downstream (pulldown, syntect) is already safe.
        let ev = match ev {
            Event::Html(s) => Event::Text(CowStr::Boxed(s.to_string().into_boxed_str())),
            Event::InlineHtml(s) => Event::Text(CowStr::Boxed(s.to_string().into_boxed_str())),
            other => other,
        };
        match &ev {
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                in_code = Some(lang);
                code_buf.clear();
            }
            Event::Text(t) if in_code.is_some() => {
                code_buf.push_str(t);
            }
            Event::End(TagEnd::CodeBlock) => {
                let lang = in_code.take().unwrap_or_default();
                // Flush anything queued so prior markdown renders first.
                if !to_render.is_empty() {
                    pulldown_cmark::html::push_html(&mut out, to_render.drain(..));
                }
                out.push_str(&super::syntax::highlight_html(&code_buf, &lang));
                code_buf.clear();
            }
            _ if in_code.is_some() => {
                // Ignore other events inside a code block; the text path above
                // is what captures the content.
            }
            _ => to_render.push(ev),
        }
    }
    if !to_render.is_empty() {
        pulldown_cmark::html::push_html(&mut out, to_render.into_iter());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_headings_and_paragraphs() {
        let html = render("# hello\n\nworld");
        assert!(html.contains("<h1>hello</h1>"));
        assert!(html.contains("<p>world</p>"));
    }

    #[test]
    fn escapes_html_in_source() {
        let html = render("safe: <script>alert(1)</script>");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn code_block_becomes_pre_code() {
        let html = render("```rust\nfn main() {}\n```");
        assert!(html.contains("<pre"), "expected pre block:\n{}", html);
        assert!(html.contains("fn"));
    }
}
