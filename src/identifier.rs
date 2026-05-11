pub fn is_valid(value: &str) -> bool {
    value.chars().enumerate().all(|(i, ch)| {
        if ch == '_' {
            return true;
        }
        if i == 0 {
            ch.is_ascii_alphabetic()
        } else {
            ch.is_ascii_alphanumeric()
        }
    })
}
