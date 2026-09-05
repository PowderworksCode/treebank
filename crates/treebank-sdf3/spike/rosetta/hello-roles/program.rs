fn fetch(url: i64, timeout: i64) -> i64 {
    while url < timeout {
        return timeout;
    }
    return url;
}

fn method(this: i64) -> i64 {
    return fetch(this, 3);
}
