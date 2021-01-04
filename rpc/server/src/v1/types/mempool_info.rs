use miner::MemoryPoolInformation;

impl From<MemoryPoolInformation> for MempoolInfo {
	fn from(info: MemoryPoolInformation) -> Self {
		MempoolInfo {
			size: info.transactions_count,
			usage: info.transactions_size_in_bytes,
			..Default::default()
		}
	}
}

#[derive(Serialize, Default)]
pub struct MempoolInfo {
	// Current tx count
	size: usize,
	// Sum of all virtual transaction sizes as defined in BIP 141. Differs from actual serialized size because witness data is discounted
	bytes: usize,
	// Total memory usage for the mempool
	usage: usize,
	// Maximum memory usage for the mempool
	maxmempool: usize,
	// Minimum fee rate in BTC/kB for tx to be accepted. Is the maximum of minrelaytxfee and minimum mempool fee
	mempoolminfee: usize,
	// Current minimum relay fee for transactions
	minrelaytxfee: usize,
}
