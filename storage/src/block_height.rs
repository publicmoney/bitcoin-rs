use bitcrypto::SHA256D;
use std::fmt;

/// Block Height
#[derive(Clone, PartialEq, Default)]
pub struct BlockHeight {
	/// Hash of the block
	pub hash: SHA256D,
	/// Height/number of the block (genesis block has zero height)
	pub number: u32,
}

impl fmt::Debug for BlockHeight {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.debug_struct("BlockHeight")
			.field("hash", &self.hash)
			.field("number", &self.number)
			.finish()
	}
}
