use std::{env, io, fs};



fn main() {
    let input = env::args().nth(1);
    let mut reader: Box<dyn io::Read> = match input {
        None => Box::new(io::stdin()),
        Some(filename)   => Box::new(fs::File::open(filename).unwrap())
    };

    let mut buffer = String::new();
    reader.read_to_string(&mut buffer);

    for line in buffer.split("\n") {
        println!("{}", line);
    }
}
