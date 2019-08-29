pub mod utils;
pub mod codec;

#[derive(Copy, Clone, Debug)]
pub struct Measurement {
    pub timestamp: u64,
    pub count: u64,
    pub value: f64
}