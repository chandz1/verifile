/// If user uploads a hash file (text) allow common formats:
/// - single hex line
/// - "filename <hash>"
/// - "hash  filename"
pub fn parse_first_hash_from_text(s: &str) -> Option<String> {
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() { continue; }
        let tokens: Vec<&str> = t.split_whitespace().collect();
        // If single token and looks hex
        if tokens.len() == 1 {
            return Some(tokens[0].to_string());
        }
        // else try second token as hex (common formats)
        for &tok in &tokens {
            if tok.chars().all(|c| c.is_ascii_hexdigit()) && tok.len() >= 16 {
                return Some(tok.to_string());
            }
        }
    }
    None
}
