use crate::impl_array_wrapper;
use bitcrypto::{dhash256, FromHex, HexError};
use std::convert::TryInto;

impl_array_wrapper!(Checksum, 4);

impl Checksum {
	pub fn generate(bytes: &[u8]) -> Checksum {
		let hash = dhash256(bytes);
		Checksum(hash[0..4].try_into().unwrap())
	}

	pub fn from_slice(bytes: &[u8]) -> Checksum {
		Checksum(bytes[0..4].try_into().unwrap())
	}
}

impl std::str::FromStr for Checksum {
	type Err = HexError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let vec: Vec<u8> = FromHex::from_hex(s)?;
		match vec.len() {
			4 => {
				let mut result = Checksum::default();
				result.copy_from_slice(&vec);
				Ok(result)
			}
			_ => Err(HexError::InvalidLength(4, vec.len())),
		}
	}
}

impl From<&'static str> for Checksum {
	fn from(s: &'static str) -> Self {
		s.parse().unwrap()
	}
}

#[test]
fn test_checksum() {
	assert_eq!(Checksum::generate(b"hello"), "9595c9df".into());
}
