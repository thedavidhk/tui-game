//! Word-wrap helpers for fixed-width text panels.

/// Greedy word-wrap by whitespace into lines of at most `line_w` characters.
#[must_use]
pub fn wrap_words(text: &str, line_w: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(w);
        } else if cur.len() + 1 + w.len() <= line_w {
            cur.push(' ');
            cur.push_str(w);
        } else {
            out.push(cur);
            cur = w.to_string();
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() && !text.is_empty() {
        out.push(text.to_string());
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}
