use crate::{Deployment, Magic, Network};
use bitcrypto::{FromStr, SHA256D};

#[derive(Debug, Clone)]
/// Parameters that influence chain consensus.
pub struct ConsensusParams {
	/// Network.
	pub network: Network,
	/// Time when BIP16 becomes active.
	/// See https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki
	pub bip16_time: u32,
	/// Block height at which BIP34 becomes active.
	/// See https://github.com/bitcoin/bips/blob/master/bip-0034.mediawiki
	pub bip34_height: u32,
	/// Block height at which BIP65 becomes active.
	/// See https://github.com/bitcoin/bips/blob/master/bip-0065.mediawiki
	pub bip65_height: u32,
	/// Block height at which BIP65 becomes active.
	/// See https://github.com/bitcoin/bips/blob/master/bip-0066.mediawiki
	pub bip66_height: u32,
	/// Version bits activation
	pub rule_change_activation_threshold: u32,
	/// Number of blocks with the same set of rules
	pub miner_confirmation_window: u32,
	/// BIP68, BIP112, BIP113 deployment
	pub csv_deployment: Option<Deployment>,
	/// BIP141, BIP143, BIP147 deployment
	pub segwit_deployment: Option<Deployment>,

	/// Absolute maximum block size.
	pub absolute_maximum_block_size: usize,
	/// Absolute maximum number of sigops in single block.
	pub absolute_maximum_block_sigops: usize,
	/// Witness scale factor
	pub witness_scale_factor: usize,
	pub max_transaction_size: usize,
	pub max_block_size: usize,
	pub max_block_sigops: usize,
	pub max_block_sigops_cost: usize,
	pub max_block_weight: usize,
	/// Number of blocks after creation that a coinbase transaction becomes spendable
	pub coinbase_maturity: u32,
}

impl ConsensusParams {
	pub fn new(network: Network) -> Self {
		match network {
			Network::Mainnet | Network::Other(_) => ConsensusParams {
				network,
				bip16_time: 1333238400, // Apr 1 2012
				bip34_height: 227931,   // 000000000000024b89b42a942fe0d9fea3bb44ab7bd1b19115dd6a759c0808b8
				bip65_height: 388381,   // 000000000000000004c2b624ed5d7756c508d90fd0da2c7c679febfa6c4735f0
				bip66_height: 363725,   // 00000000000000000379eaa19dce8c9b722d46ae6a57c2f1a988119488b50931
				segwit_deployment: Some(Deployment {
					name: "segwit",
					bit: 1,
					start_time: 1479168000,
					timeout: 1510704000,
					activation: Some(481824),
				}),
				absolute_maximum_block_size: 32_000_000,
				absolute_maximum_block_sigops: 160_000,
				witness_scale_factor: 4,
				max_transaction_size: 1_000_000,
				max_block_size: 1_000_000,
				max_block_sigops: 20_000,
				max_block_sigops_cost: 80_000,
				max_block_weight: 4_000_000,
				rule_change_activation_threshold: 1916, // 95%
				miner_confirmation_window: 2016,
				coinbase_maturity: 100, // 2 hours
				csv_deployment: Some(Deployment {
					name: "csv",
					bit: 0,
					start_time: 1462060800,
					timeout: 1493596800,
					activation: Some(419328),
				}),
			},
			Network::Testnet => ConsensusParams {
				network,
				bip16_time: 1333238400, // Apr 1 2012
				bip34_height: 21111,    // 0000000023b3a96d3484e5abb3755c413e7d41500f8e2a5c3f0dd01299cd8ef8
				bip65_height: 581885,   // 00000000007f6655f22f98e72ed80d8b06dc761d5da09df0fa1dc4be4f861eb6
				bip66_height: 330776,   // 000000002104c8c45e99a8853285a3b592602a3ccde2b832481da85e9e4ba182
				segwit_deployment: Some(Deployment {
					name: "segwit",
					bit: 1,
					start_time: 1462060800,
					timeout: 1493596800,
					activation: Some(834624),
				}),
				absolute_maximum_block_size: 32_000_000,
				absolute_maximum_block_sigops: 160_000,
				witness_scale_factor: 4,
				max_transaction_size: 1_000_000,
				max_block_size: 1_000_000,
				max_block_sigops: 20_000,
				max_block_sigops_cost: 80_000,
				max_block_weight: 4_000_000,
				rule_change_activation_threshold: 1512, // 75%
				miner_confirmation_window: 2016,
				coinbase_maturity: 100, // 2 hours
				csv_deployment: Some(Deployment {
					name: "csv",
					bit: 0,
					start_time: 1456790400,
					timeout: 1493596800,
					activation: Some(770112),
				}),
			},
			Network::Regtest | Network::Unitest => ConsensusParams {
				network,
				bip16_time: 1333238400,  // Apr 1 2012
				bip34_height: 100000000, // not activated on regtest
				bip65_height: 1351,
				bip66_height: 1251, // used only in rpc tests
				segwit_deployment: Some(Deployment {
					name: "segwit",
					bit: 1,
					start_time: 0,
					timeout: ::std::u32::MAX,
					activation: None,
				}),
				absolute_maximum_block_size: 32_000_000,
				absolute_maximum_block_sigops: 160_000,
				witness_scale_factor: 4,
				max_transaction_size: 1_000_000,
				max_block_size: 1_000_000,
				max_block_sigops: 20_000,
				max_block_sigops_cost: 80_000,
				max_block_weight: 4_000_000,
				rule_change_activation_threshold: 108, // 75%
				miner_confirmation_window: 144,
				coinbase_maturity: 100,
				csv_deployment: Some(Deployment {
					name: "csv",
					bit: 0,
					start_time: 0,
					timeout: 0,
					activation: Some(0),
				}),
			},
		}
	}

	pub fn magic(&self) -> Magic {
		self.network.magic()
	}

	pub fn is_bip30_exception(&self, hash: &SHA256D, height: u32) -> bool {
		(height == 91842 && hash == &SHA256D::from_str("00000000000a4d0a398161ffc163c503763b1f4360639393e0e4c8e300e0caec").unwrap())
			|| (height == 91880 && hash == &SHA256D::from_str("00000000000743f190a18c5577a3c2d2a1f610ae9601ac046a38084ccb7cd721").unwrap())
	}

	/// Absolute (across all forks) maximum block size.
	pub fn absolute_maximum_block_size() -> usize {
		32_000_000
	}

	/// Absolute (across all forks) maximum number of sigops in single block.
	pub fn absolute_maximum_block_sigops() -> usize {
		160_000
	}

	/// Witness scale factor (equal among all forks)
	pub fn witness_scale_factor() -> usize {
		4
	}
}

impl Default for ConsensusParams {
	fn default() -> Self {
		ConsensusParams::new(Network::default())
	}
}

#[cfg(test)]
mod tests {
	use super::super::Network;
	use super::ConsensusParams;

	#[test]
	fn test_consensus_params_bip34_height() {
		assert_eq!(ConsensusParams::new(Network::Mainnet).bip34_height, 227931);
		assert_eq!(ConsensusParams::new(Network::Testnet).bip34_height, 21111);
		assert_eq!(ConsensusParams::new(Network::Regtest).bip34_height, 100000000);
	}

	#[test]
	fn test_consensus_params_bip65_height() {
		assert_eq!(ConsensusParams::new(Network::Mainnet).bip65_height, 388381);
		assert_eq!(ConsensusParams::new(Network::Testnet).bip65_height, 581885);
		assert_eq!(ConsensusParams::new(Network::Regtest).bip65_height, 1351);
	}

	#[test]
	fn test_consensus_params_bip66_height() {
		assert_eq!(ConsensusParams::new(Network::Mainnet).bip66_height, 363725);
		assert_eq!(ConsensusParams::new(Network::Testnet).bip66_height, 330776);
		assert_eq!(ConsensusParams::new(Network::Regtest).bip66_height, 1251);
	}

	#[test]
	fn test_consensus_activation_threshold() {
		assert_eq!(ConsensusParams::new(Network::Mainnet).rule_change_activation_threshold, 1916);
		assert_eq!(ConsensusParams::new(Network::Testnet).rule_change_activation_threshold, 1512);
		assert_eq!(ConsensusParams::new(Network::Regtest).rule_change_activation_threshold, 108);
	}

	#[test]
	fn test_consensus_miner_confirmation_window() {
		assert_eq!(ConsensusParams::new(Network::Mainnet).miner_confirmation_window, 2016);
		assert_eq!(ConsensusParams::new(Network::Testnet).miner_confirmation_window, 2016);
		assert_eq!(ConsensusParams::new(Network::Regtest).miner_confirmation_window, 144);
	}
}
