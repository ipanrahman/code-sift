use crate::utils::clamp;

pub fn indent(text: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    text.lines().map(|l| format!("{}{}", pad, l)).collect::<Vec<_>>().join("\n")
}

pub fn truncate(text: &str, max_len: usize) -> &str {
    let len = text.len().min(max_len);
    &text[..len]
}
