#[macro_use]
extern crate bitcoin_hashes;
#[macro_use]
extern crate heapsize;

pub use bitcoin_hashes::core::str::FromStr;
use bitcoin_hashes::hash160;
pub use bitcoin_hashes::hex::Error as HexError;
pub use bitcoin_hashes::hex::FromHex;
use bitcoin_hashes::siphash24;
pub use bitcoin_hashes::Hash;
pub use bitcoin_hashes::HashEngine;
use bitcoin_hashes::{ripemd160, sha1, sha256, sha256d};

hash_newtype!(
	RIPEMD160,
	ripemd160::Hash,
	20,
	doc = "RIPEMD160 newtype wrapper of bitcoin_hashes::ripemd160::Hash"
);

#[inline]
pub fn ripemd160(input: &[u8]) -> RIPEMD160 {
	RIPEMD160(ripemd160::Hash::hash(input))
}

hash_newtype!(SHA1, sha1::Hash, 20, doc = "SHA1 newtype wrapper of bitcoin_hashes::sha1::Hash");

#[inline]
pub fn sha1(input: &[u8]) -> SHA1 {
	SHA1(sha1::Hash::hash(input))
}

hash_newtype!(
	SHA256,
	sha256::Hash,
	32,
	doc = "SHA256 newtype wrapper of bitcoin_hashes::sha256::Hash"
);

#[inline]
pub fn sha256(input: &[u8]) -> SHA256 {
	SHA256(sha256::Hash::hash(input))
}

hash_newtype!(
	HASH160,
	hash160::Hash,
	20,
	doc = "HASH160 newtype wrapper of bitcoin_hashes::hash160::Hash"
);

#[inline]
pub fn dhash160(input: &[u8]) -> HASH160 {
	HASH160(hash160::Hash::hash(input))
}

hash_newtype!(
	SHA256D,
	sha256d::Hash,
	32,
	doc = "SHA256D newtype wrapper of bitcoin_hashes::sha256d::Hash"
);

#[inline]
pub fn dhash256(input: &[u8]) -> SHA256D {
	SHA256D(sha256d::Hash::hash(input))
}

/// A lot of tests use hashes in the form of already reversed hex strings. SHA256D from_str/from_hex impl reverses the
/// order and we don't want to do that if it's already reversed. This trait should only be used for this one hash type.
pub trait FromInnerHex {
	fn from_inner_hex(hex: &str) -> Result<SHA256D, HexError>;
}

impl FromInnerHex for SHA256D {
	fn from_inner_hex(hex: &str) -> Result<SHA256D, HexError> {
		Ok(SHA256D::from_inner(FromHex::from_hex(hex)?))
	}
}

known_heap_size!(0, SHA256D);

#[inline]
pub fn siphash24(key0: u64, key1: u64, input: &[u8]) -> u64 {
	siphash24::Hash::hash_to_u64_with_keys(key0, key1, input)
}

#[cfg(test)]
mod tests {
	use super::{dhash160, ripemd160, sha1, sha256, siphash24, FromStr};
	use crate::{dhash256, FromInnerHex, HASH160, RIPEMD160, SHA1, SHA256, SHA256D};
	use bitcoin_hashes::hex::FromHex;

	#[test]
	fn test_ripemd160() {
		let expected = RIPEMD160::from_str("108f07b8382412612c048d07d13f814118445acd").unwrap();
		let result = ripemd160(b"hello");
		assert_eq!(result, expected);
	}

	#[test]
	fn test_sha1() {
		let expected = SHA1::from_str("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d").unwrap();
		let result = sha1(b"hello");
		assert_eq!(result, expected);
	}

	#[test]
	fn test_sha256() {
		let expected = SHA256::from_str("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824").unwrap();
		let result = sha256(b"hello");
		assert_eq!(result, expected);
	}

	#[test]
	fn test_dhash160() {
		let expected = HASH160::from_str("b6a9c8c230722b7c748331a8b450f05566dc7d0f").unwrap();
		let result = dhash160(b"hello");
		assert_eq!(result, expected);
	}

	#[test]
	fn test_dhash256() {
		let expected = SHA256D::from_inner_hex("9595c9df90075148eb06860365df33584b75bff782a510c6cd4883a419833d50").unwrap();
		let result = dhash256(b"hello");
		assert_eq!(result, expected);
	}

	#[test]
	fn test_dhash256_hex() {
		let expected = SHA256D::from_inner_hex("3bb13029ce7b1f559ef5e747fcac439f1455a2ec7c5f09b72290795e70665044").unwrap();
		let result = dhash256(&Vec::<u8>::from_hex("ffffffff").unwrap());
		assert_eq!(result, expected);
	}

	#[test]
	fn test_siphash24() {
		let expected = 0x74f839c593dc67fd_u64;
		let result = siphash24(0x0706050403020100_u64, 0x0F0E0D0C0B0A0908_u64, &[0; 1]);
		assert_eq!(result, expected);
	}
}
