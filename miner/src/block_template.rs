use bitcrypto::SHA256D;
use chain::IndexedTransaction;
use primitives::compact::Compact;

/// Block template as described in [BIP0022](https://github.com/bitcoin/bips/blob/master/bip-0022.mediawiki#block-template-request)
pub struct BlockTemplate {
	/// Version
	pub version: u32,
	/// The hash of previous block
	pub previous_header_hash: SHA256D,
	/// The current time as seen by the server
	pub time: u32,
	/// The compressed difficulty
	pub bits: Compact,
	/// Block height
	pub height: u32,
	/// Block transactions (excluding coinbase)
	pub transactions: Vec<IndexedTransaction>,
	/// Total funds available for the coinbase (in Satoshis)
	pub coinbase_value: u64,
	/// Number of bytes allowed in the block
	pub size_limit: u32,
	/// Number of sigops allowed in the block
	pub sigop_limit: u32,
}
