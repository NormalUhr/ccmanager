//! Tool-call and tool-result rendering. Filled in by task 6/7.

#![allow(dead_code)]

use crate::tool_format;
use serde_json::Value;

/// Render a tool invocation as `<details><summary>…</summary>…</details>`.
pub fn render_tool_use(name: &str, input: &Value) -> String {
    let formatted = tool_format::format_tool_call(name, input, 120);
    let header = html_escape(&formatted.header);
    let body = match formatted.body {
        Some(b) => html_escape(&b),
        None => String::new(),
    };
    format!(
        "<details class=\"tool\"><summary><span class=\"tool-name\">{name}</span> {header}</summary>\
         <pre class=\"tool-body\">{body}</pre></details>",
        name = html_escape(name),
        header = header,
        body = body,
    )
}

/// Render a tool result payload (from the user-role tool_result block).
pub fn render_tool_result(content: Option<&Value>) -> String {
    let text = match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(String::from))
            .collect::<Vec<_>>()
            .join("\n\n"),
        Some(other) => serde_json::to_string_pretty(other).unwrap_or_default(),
        None => String::new(),
    };
    let escaped = html_escape(&text);
    format!(
        "<details class=\"tool-result\"><summary>Tool result</summary>\
         <pre class=\"tool-body\">{}</pre></details>",
        escaped
    )
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
