#![allow(dead_code)]
use poke_engine::io;
use std::process::exit;

extern crate lazy_static;

#[cfg(not(feature = "system_allocator"))]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    io::main();
    exit(1);
}
