#[macro_use]
extern crate log;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rand::Rng;

const MAX: i32 = 3;

fn dine(i: i32, first: Arc<Mutex<i32>>, second: Arc<Mutex<i32>>) {
    debug!("philosopher-{} says hello", i);
    let mut rng = rand::thread_rng();
    loop {
        let pause = rng.gen_range(1..5);
        let duration = Duration::from_secs(pause);
        debug!("philosopher-{} thinking for {} sec", i, pause);
        thread::sleep(duration);
    
        debug!("philosopher-{} is hungry", i);
        {
            debug!("philosopher-{} getting first fork", i);
            let m1 = first.lock().unwrap();
            debug!("philosopher-{} got fork-{}", i, m1);
            thread::sleep(duration);

            debug!("philosopher-{} getting second fork", i);
            let m2 = second.lock().unwrap();
            debug!("philosopher-{} got fork-{}", i, m2);
            thread::sleep(duration);

            debug!("philosopher-{} eats for {} sec with {} {}", i, pause, m1, m2);
            thread::sleep(duration);
            debug!("philosopher-{} ate for {} sec with {} {}", i, pause, m1, m2);
        }
    }
}

fn main() {
    env_logger::init();

    let mutex: Vec<Arc<Mutex<i32>>> = (0..MAX).map(|i| {
            Arc::new(Mutex::new(i))
        }).collect::<Vec<_>>();


    let mut threads = Vec::new();

    for i in 0..MAX {
        let first = Arc::clone(&mutex[i as usize]);
        let second = Arc::clone(&mutex[((i + 1) % MAX) as usize]);
        let t = thread::spawn(move || {
            dine(i, first, second);
        });
        threads.push(t);
    }

    for thread in threads {
        thread.join().unwrap();
    }
}
