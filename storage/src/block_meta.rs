/// Block meta information
#[derive(Clone, PartialEq, Debug, Serializable, Deserializable)]
pub struct BlockMeta {
	/// Height/number of the block (genesis block has zero height)
	pub number: u32,
	/// Number of transactions in the block
	pub n_tx: u32,
	/// Number of transactions in the chain up to and including this block
	pub n_chain_tx: u32,
	// Total difficulty so far
	// pub cumulative_difficulty: Compact
	// Total supply
	pub total_supply: u64,
}

impl Default for BlockMeta {
	fn default() -> Self {
		BlockMeta {
			number: u32::max_value(),
			total_supply: 0,
			n_tx: 0,
			n_chain_tx: 0,
		}
	}
}
