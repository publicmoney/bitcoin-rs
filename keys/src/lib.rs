//! Bitcoin keys.

extern crate rand;
extern crate rustc_hex as hex;
#[macro_use]
extern crate lazy_static;
extern crate base58;
extern crate primitives;
extern crate secp256k1;

mod address;
mod display;
mod error;
pub mod generator;
mod keypair;
mod network;
mod private;
mod public;
mod signature;

pub use primitives::{bytes, hash};

pub use address::{Address, Type};
use bitcrypto::{HASH160, SHA256D};
pub use display::DisplayLayout;
pub use error::Error;
pub use keypair::KeyPair;
pub use network::Network;
pub use private::Private;
pub use public::Public;
pub use signature::{CompactSignature, Signature};

/// 20 bytes long hash derived from public `ripemd160(sha256(public))`
pub type AddressHash = HASH160;
/// 32 bytes long secret key
pub type Secret = [u8; 32];
/// 32 bytes long signable message
pub type Message = SHA256D;

lazy_static! {
	pub static ref SECP256K1: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
}
