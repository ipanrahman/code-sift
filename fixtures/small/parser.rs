pub fn parse_input(s: &str) -> Vec<&str> {
    s.lines().collect()
}

pub fn parse_number(s: &str) -> Option<i32> {
    s.trim().parse().ok()
}
