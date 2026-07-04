pub mod outdoor;
pub mod linky;

// pub use outdoor::Outdoor;
pub use outdoor::get_outdoor;
pub use outdoor::set_humidity_and_temperature;
pub use outdoor::set_pressure;
// pub use linky::Linky;
pub use linky::get as get_linky;
pub use linky::set as set_linky;
