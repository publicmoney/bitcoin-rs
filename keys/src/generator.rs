use crate::network::Network;
use crate::{Error, KeyPair, SECP256K1};
use secp256k1::rand::rngs::StdRng;
use secp256k1::rand::FromEntropy;

pub trait Generator {
	fn generate(&self) -> Result<KeyPair, Error>;
}

pub struct Random {
	network: Network,
}

impl Random {
	pub fn new(network: Network) -> Self {
		Random { network }
	}
}

impl Generator for Random {
	fn generate(&self) -> Result<KeyPair, Error> {
		let context = &SECP256K1;

		let mut rng = StdRng::from_entropy();
		let (secret, public) = context.generate_keypair(&mut rng);
		Ok(KeyPair::from_keypair(secret, public, self.network))
	}
}
