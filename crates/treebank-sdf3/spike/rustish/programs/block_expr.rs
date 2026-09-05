fn main() {
    let x = 1;
    let y = {
        let x = 5;
        x + 1
    };
    println!("{}", y + x);
    let x = { x + y };
    println!("{}", x);
}
