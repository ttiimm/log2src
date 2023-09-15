#[macro_use]
extern crate log;


fn main() {
    env_logger::init();
    debug!("Hello from main");    
    foo();
}

fn foo() {
    debug!("Hello from foo");
}
