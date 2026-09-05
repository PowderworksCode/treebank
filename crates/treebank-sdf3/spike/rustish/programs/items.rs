fn main() {
    println!("{}", twice(4));
    fn twice(n: i64) -> i64 {
        n * 2
    }
    println!("{}", later(1));
}
fn later(n: i64) -> i64 {
    n + 100
}
