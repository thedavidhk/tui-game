//! Word-wrap helpers for fixed-width text panels.

/// Hard-break `line` into chunks of at most `line_w` **Unicode scalar values** (never splits a `char`).
#[must_use]
fn hard_wrap_chars(line: &str, line_w: usize) -> Vec<String> {
    if line_w == 0 {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut n = 0usize;
    for ch in line.chars() {
        if n >= line_w && !cur.is_empty() {
            out.push(cur);
            cur = String::new();
            n = 0;
        }
        cur.push(ch);
        n += 1;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Expand logical HUD rows into rows that fit a terminal panel of width `line_w`.
///
/// Empty strings become blank spacer rows. If a line's **character count** is already ≤ `line_w`,
/// it is copied unchanged (preserves spacing such as `Mode     Explore`). Otherwise lines are
/// word-wrapped with [`wrap_words`], then any segment still wider than `line_w` is split with
/// [`hard_wrap_chars`].
#[must_use]
pub fn wrap_panel_lines(lines: &[String], line_w: usize) -> Vec<String> {
    let line_w = line_w.max(1);
    let mut out = Vec::new();
    for line in lines {
        if line.is_empty() {
            out.push(String::new());
            continue;
        }
        if line.chars().count() <= line_w {
            out.push(line.clone());
            continue;
        }
        let wrapped = wrap_words(line.as_str(), line_w);
        for segment in wrapped {
            let broken = hard_wrap_chars(&segment, line_w);
            out.extend(broken);
        }
    }
    out
}

/// Greedy word-wrap by whitespace into lines of at most `line_w` characters.
#[must_use]
pub fn wrap_words(text: &str, line_w: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(w);
        } else if cur.chars().count() + 1 + w.chars().count() <= line_w {
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

#[cfg(test)]
mod tests {
    use super::{wrap_panel_lines, wrap_words};

    #[test]
    fn wrap_words_counts_characters_not_bytes() {
        // Cyrillic words are 2 bytes/char; byte-based width would wrap too early.
        let out = wrap_words("ыыы ыыы", 7);
        assert_eq!(out, vec!["ыыы ыыы".to_string()]);
        let out = wrap_words("ыыы ыыы", 6);
        assert_eq!(out, vec!["ыыы".to_string(), "ыыы".to_string()]);
    }

    #[test]
    fn wrap_panel_preserves_blank_rows() {
        let lines = vec!["Mode     Explore".into(), String::new(), "Here".into()];
        let out = wrap_panel_lines(&lines, 80);
        assert_eq!(
            out,
            vec!["Mode     Explore".into(), String::new(), "Here".into()]
        );
    }

    #[test]
    fn wrap_panel_wraps_long_line() {
        let lines = vec!["Only darkness returns your gaze from below.".into()];
        let out = wrap_panel_lines(&lines, 12);
        assert_eq!(
            out,
            vec![
                "Only".to_string(),
                "darkness".to_string(),
                "returns your".to_string(),
                "gaze from".to_string(),
                "below.".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_panel_hard_breaks_single_long_token() {
        let lines = vec!["abcdefghijklmnop".into()];
        let out = wrap_panel_lines(&lines, 8);
        assert_eq!(out, vec!["abcdefgh".to_string(), "ijklmnop".to_string(),]);
    }
}
