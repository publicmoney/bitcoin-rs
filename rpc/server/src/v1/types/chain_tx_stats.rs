#[derive(Serialize, Deserialize, Default)]
pub struct ChainTxStats {
	// The timestamp for the final block in the window in UNIX format.
	pub time: usize,
	// The total number of transactions in the chain up to that point.
	pub txcount: usize,
	// The hash of the final block in the window.
	pub window_final_block_hash: String,
	// Size of the window in number of blocks.
	pub window_block_count: usize,
	// The number of transactions in the window. Only returned if "window_block_count" is > 0.
	pub window_tx_count: usize,
	// The elapsed time in the window in seconds. Only returned if "window_block_count" is > 0.
	pub window_interval: usize,
	//  The average rate of transactions per second in the window. Only returned if "window_interval" is > 0.
	pub txrate: usize,
}
