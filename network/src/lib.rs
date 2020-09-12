#[macro_use]
extern crate lazy_static;

extern crate chain;
extern crate primitives;

mod consensus;
mod deployments;
pub mod network;

pub use primitives::{compact, hash};

pub use crate::network::{Magic, Network};
pub use consensus::ConsensusParams;
pub use deployments::Deployment;
