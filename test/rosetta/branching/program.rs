fn classify(n: i32) -> &'static str {
    if n > 0 {
        return "positive";
    } else {
        return "other";
    }
}
