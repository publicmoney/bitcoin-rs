use crate::{AddressHash, CompactSignature, Error, Message, Signature, SECP256K1};
use bitcrypto::dhash160;
use hex::ToHex;
use secp256k1::key;
use secp256k1::recovery::{RecoverableSignature, RecoveryId};
use secp256k1::{Error as SecpError, Message as SecpMessage, Signature as SecpSignature};
use std::{fmt, ops};

/// Secret public key
pub enum Public {
	/// Normal version of public key (0x04 byte + X and Y coordinate on curve)
	Normal([u8; 65]),
	/// Compressed version of public key (0x02 byte for even value of Y, 0x03 byte for odd value of Y + X coordinate)
	Compressed([u8; 33]),
}

impl Public {
	pub fn from_slice(data: &[u8]) -> Result<Self, Error> {
		match data.len() {
			33 => {
				let mut pk = [0; 33];
				pk.copy_from_slice(data);
				Ok(Public::Compressed(pk))
			}
			65 => {
				let mut pk = [0; 65];
				pk.copy_from_slice(data);
				Ok(Public::Normal(pk))
			}
			_ => Err(Error::InvalidPublic),
		}
	}

	pub fn address_hash(&self) -> AddressHash {
		dhash160(self)
	}

	pub fn verify(&self, message: &Message, signature: &Signature) -> Result<bool, Error> {
		let context = &SECP256K1;
		let public = key::PublicKey::from_slice(self)?;
		let mut signature = SecpSignature::from_der_lax(signature)?;
		signature.normalize_s();
		let message = SecpMessage::from_slice(message)?;
		match context.verify(&message, &signature, &public) {
			Ok(_) => Ok(true),
			Err(SecpError::IncorrectSignature) => Ok(false),
			Err(x) => Err(x.into()),
		}
	}

	pub fn recover_compact(message: &Message, signature: &CompactSignature) -> Result<Self, Error> {
		let context = &SECP256K1;
		let recovery_id = (signature[0] - 27) & 3;
		let compressed = (signature[0] - 27) & 4 != 0;
		let recovery_id = RecoveryId::from_i32(recovery_id as i32)?;
		let signature = RecoverableSignature::from_compact(&signature[1..65], recovery_id)?;
		let message = SecpMessage::from_slice(message)?;
		let pubkey = context.recover(&message, &signature)?;

		let public = if compressed {
			let serialized = pubkey.serialize();
			Public::Compressed(serialized)
		} else {
			let serialized = pubkey.serialize_uncompressed();
			Public::Normal(serialized)
		};
		Ok(public)
	}
}

impl ops::Deref for Public {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		match *self {
			Public::Normal(ref bytes) => bytes,
			Public::Compressed(ref bytes) => bytes,
		}
	}
}

impl PartialEq for Public {
	fn eq(&self, other: &Self) -> bool {
		let s_slice: &[u8] = self;
		let o_slice: &[u8] = other;
		s_slice == o_slice
	}
}

impl fmt::Debug for Public {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Public::Normal(ref bytes) => writeln!(f, "normal: {}", bytes.to_hex::<String>()),
			Public::Compressed(ref bytes) => writeln!(f, "compressed: {}", bytes.to_hex::<String>()),
		}
	}
}

impl fmt::Display for Public {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.to_hex::<String>().fmt(f)
	}
}
