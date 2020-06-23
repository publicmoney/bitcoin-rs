use crate::network::Network;
use crate::{Error, KeyPair, SECP256K1};
use rand::rngs::OsRng;

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
		let mut rng = OsRng::new().map_err(|_| Error::FailedKeyGeneration)?;
		let (secret, public) = context.generate_keypair(&mut rng);
		Ok(KeyPair::from_keypair(secret, public, self.network))
	}
}
