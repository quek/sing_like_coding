pub fn is_subsequence_case_insensitive(name: &str, query: &str) -> bool {
    let mut query_chars = query.chars().map(|c| c.to_ascii_lowercase());
    let mut current_q = query_chars.next();

    for c in name.chars() {
        if let Some(qc) = current_q {
            if qc == c.to_ascii_lowercase() {
                current_q = query_chars.next();
            }
        } else {
            break;
        }
    }
    current_q.is_none()
}
