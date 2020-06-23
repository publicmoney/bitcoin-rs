use crate::hash::{H264, H520};
use crate::{AddressHash, CompactSignature, Error, Message, Signature, SECP256K1};
use crypto::dhash160;
use hex::ToHex;
use secp256k1::key;
use secp256k1::recovery::{RecoverableSignature, RecoveryId};
use secp256k1::{Error as SecpError, Message as SecpMessage, Signature as SecpSignature};
use std::{fmt, ops};

/// Secret public key
pub enum Public {
	/// Normal version of public key
	Normal(H520),
	/// Compressed version of public key
	Compressed(H264),
}

impl Public {
	pub fn from_slice(data: &[u8]) -> Result<Self, Error> {
		match data.len() {
			33 => {
				let mut public = H264::default();
				public.copy_from_slice(data);
				Ok(Public::Compressed(public))
			}
			65 => {
				let mut public = H520::default();
				public.copy_from_slice(data);
				Ok(Public::Normal(public))
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
		let message = SecpMessage::from_slice(&**message)?;
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
		let message = SecpMessage::from_slice(&**message)?;
		let pubkey = context.recover(&message, &signature)?;

		let public = if compressed {
			let serialized = pubkey.serialize();
			let mut public = H264::default();
			public.copy_from_slice(&serialized);
			Public::Compressed(public)
		} else {
			let serialized = pubkey.serialize_uncompressed();
			let mut public = H520::default();
			public.copy_from_slice(&serialized);
			Public::Normal(public)
		};
		Ok(public)
	}
}

impl ops::Deref for Public {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		match *self {
			Public::Normal(ref hash) => &**hash,
			Public::Compressed(ref hash) => &**hash,
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
			Public::Normal(ref hash) => writeln!(f, "normal: {}", hash.to_hex::<String>()),
			Public::Compressed(ref hash) => writeln!(f, "compressed: {}", hash.to_hex::<String>()),
		}
	}
}

impl fmt::Display for Public {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.to_hex::<String>().fmt(f)
	}
}
