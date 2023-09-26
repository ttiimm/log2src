#[macro_use]
extern crate log;


fn main() {
    env_logger::init();
    debug!("Hello from main");
    for i in 0..3 {
        foo(i);
    }
}

fn foo(i: u32) {
    debug!("Hello from foo i={}", i);
}
