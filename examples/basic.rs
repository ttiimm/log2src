#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    debug!("Hello from main");
    for i in 0..3 {
        foo(i);
    }
    bar(4);
    baz(5, 6);
}

fn foo(i: u32) {
    debug!("Hello from foo i={}", i);
}

fn bar(j: u32) { debug!("Hello from bar j={j}"); }

fn baz(i: u32, j: u32) { debug!("Hello from baz i={1} j={0}", j, i); }
