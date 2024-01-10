#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    debug!("Hello from main");
    a();
}

fn a() {
    b();
}

fn b() {
    debug!("Hello from b");
}
