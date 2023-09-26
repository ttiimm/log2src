#[macro_use]
extern crate log;

use rand::Rng;


#[derive(Debug)]
enum Animal {
    Sheep, Dog, Cow, Chicken, Goat, Horse,
}

impl Animal {
    fn sound(&self) -> &str {
        match *self {
            Animal::Sheep => "baa",
            Animal::Dog => "baa",
            Animal::Cow => "baa",
            Animal::Chicken => "baa",
            Animal::Goat => "baa",
            Animal::Horse => "baa",
        }
    }
}

fn main() {
    env_logger::init();
    let mut rng = rand::thread_rng();
    let barn = init();
    for _ in 0..10 {
        let i = rng.gen_range(0..barn.len());
        debug!("Animal is going to make a noise");
        let animal = &barn[i];
        make_noise(animal);
    }
}

fn init() -> [Animal; 6] {
    debug!("Initializing animals");
    [Animal::Sheep, Animal::Dog, Animal::Cow, Animal::Chicken, Animal::Goat, Animal::Horse]
}

fn make_noise(_: &Animal) {
    debug!("Animal says something");
}