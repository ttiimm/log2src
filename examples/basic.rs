#[macro_use]
extern crate log;

fn foo() {
    debug!("Hello from foo");
}

fn main() {
    debug!("Hello from main");    
    foo();
}