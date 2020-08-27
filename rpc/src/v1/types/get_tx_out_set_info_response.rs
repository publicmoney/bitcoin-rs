use bitcrypto::SHA256D;

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTxOutSetInfoResponse {
	// The current block height (index)
	pub height: u32,
	// The hash of the block at the tip of the chain
	pub bestblock: SHA256D,
	//The number of transactions with unspent outputs
	pub transactions: u64,
	// The number of unspent transaction outputs
	pub txouts: u64,
	// A meaningless metric for UTXO set size
	pub bogosize: u64,
	// The serialized hash
	pub hash_serialized_2: SHA256D,
	// The estimated size of the chainstate on disk
	pub disk_size: u64,
	// The total amount
	pub total_amount: f64,
}
