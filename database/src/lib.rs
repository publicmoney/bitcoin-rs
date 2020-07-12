pub mod adapator;
mod db_block;
pub use hammersbald::{persistent, transient};

extern crate parking_lot;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serialization_derive;
