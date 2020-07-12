use std::fmt;
// use primitives::compact::Compact;

/// Block meta information
#[derive(Clone, PartialEq, Default, Serializable, Deserializable)]
pub struct BlockMeta {
	/// Height/number of the block (genesis block has zero height)
	pub number: u32,
	/// Number of transactions in the block
	pub n_tx: u32,
	/// Number of transactions in the chain up to and including this block
	pub n_chain_tx: u32,
	// Total difficulty so far
	// pub cumulative_difficulty: Compact
}

impl fmt::Debug for BlockMeta {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.debug_struct("BlockMeta")
			.field("number", &self.number)
			.field("n_tx", &self.n_tx)
			.field("n_chain_tx", &self.n_chain_tx)
			.finish()
	}
}
