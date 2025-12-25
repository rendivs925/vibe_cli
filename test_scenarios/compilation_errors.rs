// Test file with compilation errors
fn main() {
    // Undefined variable error
    let result = undefined_variable + 1;

    // Type mismatch error
    let x: i32 = "string";

    // Borrow checker error - use after move
    let s = String::from("hello");
    let t = s;
    println!("{}", s); // s has been moved

    // Unused variable warning
    let unused_var = 42;

    // Index out of bounds potential
    let arr = [1, 2, 3];
    let idx = 10;
    println!("{}", arr[idx]); // Will panic at runtime
}