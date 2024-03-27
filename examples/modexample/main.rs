#[macro_use]
extern crate log;

mod garden;

fn main() {
    env_logger::init();
    debug!("hello gardener");
    let garden = garden::plant();
    let veggies = garden::gather(garden);
    debug!("Veggies were {:?}", veggies);
}
