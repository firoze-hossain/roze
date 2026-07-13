// Roze Runtime Library
// This will contain the runtime support for Roze programs

pub mod gc;
pub mod memory;
pub mod threading;

pub fn init() {
    println!("🌹 Roze Runtime v0.1 initialized");
}