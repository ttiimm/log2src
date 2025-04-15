#[macro_use]
extern crate log;

use rand::Rng;

#[derive(Debug)]
enum Animal {
    Sheep,
    Dog,
    Cow,
    Chicken,
    Goat,
    Horse,
}

impl Animal {
    fn sound(&self) -> &str {
        match *self {
            Animal::Sheep => "baa",
            Animal::Dog => "woof",
            Animal::Cow => "moo",
            Animal::Chicken => "cluck",
            Animal::Goat => "bleaah",
            Animal::Horse => "neigh",
        }
    }
}

fn main() {
    env_logger::init();
    let mut rng = rand::rng();
    let barn = init();
    for _ in 0..10 {
        let i = rng.random_range(0..barn.len());
        debug!("Animal is going to make a noise");
        let animal = &barn[i];
        make_noise(animal);
    }
}

fn init() -> [Animal; 6] {
    debug!("Initializing animals");
    [
        Animal::Sheep,
        Animal::Dog,
        Animal::Cow,
        Animal::Chicken,
        Animal::Goat,
        Animal::Horse,
    ]
}

fn make_noise(animal: &Animal) {
    let sound = animal.sound();
    debug!("Animal {:?} says {:?}", animal, sound);
}
