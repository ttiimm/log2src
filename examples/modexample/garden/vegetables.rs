use std::fmt::{Debug, Formatter, Result};

pub trait Vegetable {
    fn identify(&self) -> &'static str;
}

impl Debug for dyn Vegetable {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", self.identify())
    }
}

pub struct Asparagus;

impl Vegetable for Asparagus {
    fn identify(&self) -> &'static str {
        let name = "asparagus";
        debug!("Trying to identify {}", name);
        name
    }
}

pub struct Brocolli;

impl Vegetable for Brocolli {
    fn identify(&self) -> &'static str {
        let name = "brocolli";
        debug!("Trying to identify {}", name);
        name
    }
}

pub struct Carrot;

impl Vegetable for Carrot {
    fn identify(&self) -> &'static str {
        let name = "carrot";
        debug!("Trying to identify {}", name);
        name
    }
}

pub struct Dill;

impl Vegetable for Dill {
    fn identify(&self) -> &'static str {
        let name = "dill";
        debug!("Trying to identify {}", name);
        name
    }
}
