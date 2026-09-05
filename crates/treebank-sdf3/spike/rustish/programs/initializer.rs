fn main() {
    let a = 2;
    let b = a * 3;
    let a = b - a;
    let b = {
        let a = a + 1;
        a * b
    };
    println!("{}", a);
    println!("{}", b);
}
