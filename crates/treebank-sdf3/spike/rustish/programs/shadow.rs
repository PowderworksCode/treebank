fn main() {
    let x = 1;
    println!("{}", x);
    let x = x + 10;
    println!("{}", x);
    {
        let x = x * 2;
        println!("{}", x);
    }
    println!("{}", x);
}
