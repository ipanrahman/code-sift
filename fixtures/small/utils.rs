pub fn format_output(a: i32, b: i32) -> String {
    format!("sum={}, product={}", a, b)
}

pub fn clamp(value: i32, min: i32, max: i32) -> i32 {
    if value < min { min } else if value > max { max } else { value }
}
