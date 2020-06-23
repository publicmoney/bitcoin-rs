use crate::v1::types::H256;
use std::collections::HashMap;

/// Verbose block information
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BlockchainInfo {
	/// Chain this node is on
	pub chain: String,
	/// Number of blocks in chain
	pub blocks: u32,
	/// Number of headers in chain
	pub headers: u32,
	/// Hash of the best block
	pub bestblockhash: Option<H256>,
	/// Cumulative difficulty of the chain
	pub difficulty: Option<f64>,
	/// Median time
	pub mediantime: u32,
	/// Progress of chain verification (0-1)
	pub verificationprogress: f32,
	/// Whether initial block download is in progress
	pub initialblockdownload: bool,
	/// Chainwork
	pub chainwork: H256,
	/// Amount of storage used by the chain
	pub size_on_disk: u64,
	/// Whether the chain is pruned
	pub pruned: bool,
	/// Supported soft forks
	pub softforks: HashMap<String, SoftFork>,
	/// Warnings
	pub warnings: String,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SoftFork {
	pub f_type: String,
	pub active: String,
	pub height: String,
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json;

	fn info() -> BlockchainInfo {
		BlockchainInfo {
			chain: "main".to_string(),
			blocks: 1,
			headers: 3,
			bestblockhash: Some(H256::from(1)),
			difficulty: Some(1.0),
			mediantime: 5000,
			verificationprogress: 1.0,
			initialblockdownload: false,
			chainwork: H256::from(1),
			size_on_disk: 1000,
			pruned: false,
			softforks: HashMap::new(),
			warnings: "".to_string(),
		}
	}

	fn info_str() -> &'static str {
		r#"{"chain":"main","blocks":1,"headers":3,"bestblockhash":"0100000000000000000000000000000000000000000000000000000000000000","difficulty":1.0,"mediantime":5000,"verificationprogress":1.0,"initialblockdownload":false,"chainwork":"0100000000000000000000000000000000000000000000000000000000000000","size_on_disk":1000,"pruned":false,"softforks":{},"warnings":""}"#
	}

	#[test]
	fn tx_out_response_serialize() {
		let info = info();
		assert_eq!(serde_json::to_string(&info).unwrap(), info_str());
	}

	#[test]
	fn tx_out_response_deserialize() {
		let info = info();
		assert_eq!(serde_json::from_str::<BlockchainInfo>(info_str()).unwrap(), info);
	}
}
