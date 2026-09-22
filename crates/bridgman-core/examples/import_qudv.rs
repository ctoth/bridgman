use std::{env, fs};

fn main() {
    let path = env::args().nth(1).expect("usage: import_qudv CATALOG.yml");
    let source = fs::read_to_string(path).expect("read catalog");
    let declarations = bridgman_core::qudv_schema2_to_catalog(&source).expect("adapt catalog");
    let registry = bridgman_core::Registry::compile(declarations).expect("compile catalog");
    println!(
        "{} kinds, {} units",
        registry.kinds().len(),
        registry.units().len()
    );
}
