/// Block meta information
#[derive(Clone, PartialEq, Debug, Serializable, Deserializable)]
pub struct BlockMeta {
	/// Height/number of the block (genesis block has zero height)
	pub number: u32,
	/// Number of transactions in the block
	pub n_tx: u32,
	/// Total number of transactions in the chain up to and including this block
	pub n_chain_tx: u64,
	/// Total number of transactions with UTXOs
	pub n_tx_with_utxos: u64,
	// Total difficulty so far
	// pub cumulative_difficulty: U256, ?
	// Total bytes used to store the blockchain
	pub total_size: u64,
	// Total supply
	pub total_supply: u64,
}

impl Default for BlockMeta {
	fn default() -> Self {
		BlockMeta {
			number: u32::max_value(),
			n_tx: 0,
			n_chain_tx: 0,
			n_tx_with_utxos: 0,
			total_size: 0,
			total_supply: 0,
		}
	}
}
