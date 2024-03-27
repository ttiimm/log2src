use crate::garden::vegetables::{Asparagus, Brocolli, Carrot, Dill, Vegetable};

mod vegetables;

pub struct Garden {
    plants: Vec<Box<dyn Vegetable>>
}

impl Garden {
    fn new() -> Garden {
        let plants = vec![
            Box::new(Asparagus) as Box<dyn Vegetable>,
            Box::new(Brocolli) as Box<dyn Vegetable>,
            Box::new(Carrot) as Box<dyn Vegetable>,
            Box::new(Dill) as Box<dyn Vegetable>,
        ];
        debug!("Creating garden with {:?}", plants);
        Garden { plants }
    }
}

pub fn plant() -> Garden {
    debug!("Planting garden");
    Garden::new()
}

pub fn gather(garden: Garden) -> Vec<&'static str> {
    debug!("fetching veggies");
    let mut harvested = vec![];
    for veg in garden.plants {
        let identity = veg.identify();
        debug!("Vegetable was {}", identity);
        harvested.push(identity);
    }
    harvested
}
