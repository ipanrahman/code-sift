mod utils;
mod math;
mod io;

use utils::format_output;
use math::{add, multiply};
use io::read_file;

fn main() {
    let a = 10;
    let b = 20;
    let sum = add(a, b);
    let product = multiply(a, b);
    let output = format_output(sum, product);
    println!("{}", output);
}

fn helper() {
    let x = add(1, 2);
    multiply(x, 3);
}
