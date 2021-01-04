use crate::v1::helpers::errors::{
	block_at_height_not_found, block_not_found, transaction_not_found, transaction_of_side_branch, transaction_output_not_found, unknown,
};
use crate::v1::traits::BlockChain;
use crate::v1::types::ChainTxStats;
use crate::v1::types::U256;
use crate::v1::types::{BlockchainInfo, GetTxOutSetInfoResponse};
use crate::v1::types::{GetBlockResponse, RawBlock, VerboseBlock};
use crate::v1::types::{GetTxOutResponse, TransactionOutputScript};
use bitcrypto::SHA256D;
use chain::OutPoint;
use global_script::Script;
use jsonrpc_core::Error;
use keys::{self, Address};
use network::Network;
use ser::serialize;
use std::collections::HashMap;
use storage;
use storage::{BlockMeta, BlockRef};
use verification;
use verification::constants::TARGET_SPACING_SECONDS;

pub struct BlockChainClient<T: BlockChainClientCoreApi> {
	core: T,
}

pub trait BlockChainClientCoreApi: Send + Sync + 'static {
	fn is_synchronizing(&self) -> bool;
	fn synchronization_progress(&self) -> f32;
	fn size_on_disk(&self) -> u64;
	fn network(&self) -> String;
	fn header_count(&self) -> u32;
	fn best_block_hash(&self) -> SHA256D;
	fn block_count(&self) -> u32;
	fn block_hash(&self, height: u32) -> Option<SHA256D>;
	fn difficulty(&self) -> f64;
	fn median_time(&self) -> u32;
	fn block_meta(&self, hash: SHA256D) -> Option<BlockMeta>;
	fn raw_block(&self, hash: SHA256D) -> Option<RawBlock>;
	fn verbose_block(&self, hash: SHA256D) -> Option<VerboseBlock>;
	fn verbose_transaction_out(&self, prev_out: OutPoint) -> Result<GetTxOutResponse, Error>;
	fn chain_tx_stats(&self, nblocks: Option<usize>, blockhash: Option<String>) -> Result<ChainTxStats, Error>;
}

pub struct BlockChainClientCore {
	network: Network,
	storage: storage::SharedStore,
	local_sync_node: Option<sync::LocalNodeRef>, // Might be None only in tests.
}

impl BlockChainClientCore {
	pub fn new(network: Network, storage: storage::SharedStore, local_sync_node: Option<sync::LocalNodeRef>) -> Self {
		BlockChainClientCore {
			network,
			storage,
			local_sync_node,
		}
	}
}

impl BlockChainClientCoreApi for BlockChainClientCore {
	fn is_synchronizing(&self) -> bool {
		self.local_sync_node.as_ref().unwrap().sync_state().synchronizing()
	}

	fn synchronization_progress(&self) -> f32 {
		self.local_sync_node.as_ref().unwrap().sync_state().synchronization_progress()
	}

	fn size_on_disk(&self) -> u64 {
		self.storage.size()
	}

	fn network(&self) -> String {
		self.network.to_string()
	}

	fn header_count(&self) -> u32 {
		self.block_count() + self.local_sync_node.as_ref().unwrap().information().chain.headers.best
	}

	fn best_block_hash(&self) -> SHA256D {
		self.storage.best_block().hash
	}

	fn block_count(&self) -> u32 {
		self.storage.best_block().number
	}

	fn block_hash(&self, height: u32) -> Option<SHA256D> {
		self.storage.block_hash(height)
	}

	fn difficulty(&self) -> f64 {
		self.storage.difficulty()
	}

	fn median_time(&self) -> u32 {
		verification::median_timestamp(&self.storage.best_header().raw, self.storage.as_block_header_provider())
	}

	fn raw_block(&self, hash: SHA256D) -> Option<RawBlock> {
		self.storage.block(hash.into()).map(|block| serialize(&block.to_raw_block()).into())
	}

	fn verbose_block(&self, hash: SHA256D) -> Option<VerboseBlock> {
		self.storage.block(hash.into()).map(|block| {
			let height = self.storage.block_number(block.hash());
			let confirmations = match height {
				Some(block_number) => (self.storage.best_block().number - block_number + 1) as i64,
				None => -1,
			};
			let strippedsize = block.size() as u32;
			let size = block.size_with_witness() as u32;
			let weight = block.weight() as u32;
			let median_time = verification::median_timestamp(&block.header.raw, self.storage.as_block_header_provider());

			VerboseBlock {
				confirmations,
				size,
				strippedsize,
				weight,
				height,
				mediantime: Some(median_time),
				difficulty: block.header.raw.bits.to_f64(),
				chainwork: U256::default(), // TODO: read from storage
				previousblockhash: Some(block.header.raw.previous_header_hash.clone().into()),
				nextblockhash: height.and_then(|h| self.storage.block_hash(h + 1).map(|h| h.into())),
				bits: block.header.raw.bits.into(),
				hash: block.hash().clone().into(),
				merkleroot: block.header.raw.merkle_root_hash.clone().into(),
				nonce: block.header.raw.nonce,
				time: block.header.raw.time,
				tx: block.transactions.into_iter().map(|t| t.hash.into()).collect(),
				version: block.header.raw.version,
				version_hex: format!("{:x}", &block.header.raw.version),
			}
		})
	}

	fn block_meta(&self, hash: SHA256D) -> Option<BlockMeta> {
		self.storage.block_meta(BlockRef::Hash(hash))
	}

	fn verbose_transaction_out(&self, prev_out: OutPoint) -> Result<GetTxOutResponse, Error> {
		let transaction = match self.storage.transaction(&prev_out.hash) {
			Some(transaction) => transaction,
			// no transaction => no response
			None => return Err(transaction_not_found(prev_out.hash)),
		};

		if prev_out.index >= transaction.raw.outputs.len() as u32 {
			return Err(transaction_output_not_found(prev_out));
		}

		let meta = match self.storage.transaction_meta(&prev_out.hash) {
			Some(meta) => meta,
			// not in the main branch => no response
			None => return Err(transaction_of_side_branch(prev_out.hash)),
		};

		let block_header = match self.storage.block_header(meta.height().into()) {
			Some(block_header) => block_header,
			// this is possible during reorgs
			None => return Err(transaction_not_found(prev_out.hash)),
		};

		let best_block = self.storage.best_block();
		if best_block.number < meta.height() {
			// this is possible during reorgs
			return Err(transaction_not_found(prev_out.hash));
		}

		let ref script_bytes = transaction.raw.outputs[prev_out.index as usize].script_pubkey;
		let script: Script = script_bytes.clone().into();
		let script_asm = format!("{}", script);
		let script_addresses = script.extract_destinations().unwrap_or(vec![]);

		Ok(GetTxOutResponse {
			bestblock: block_header.hash.into(),
			confirmations: best_block.number - meta.height() + 1,
			value: 0.00000001f64 * (transaction.raw.outputs[prev_out.index as usize].value as f64),
			script: TransactionOutputScript {
				asm: script_asm,
				hex: script_bytes.clone().into(),
				req_sigs: script.num_signatures_required() as u32,
				script_type: script.script_type().into(),
				addresses: script_addresses
					.into_iter()
					.map(|a| Address {
						network: match self.network {
							Network::Mainnet => keys::Network::Mainnet,
							// there's no correct choices for Regtests && Other networks
							// => let's just make Testnet key
							_ => keys::Network::Testnet,
						},
						hash: a.hash,
						kind: a.kind,
					})
					.collect(),
			},
			version: transaction.raw.version,
			coinbase: transaction.raw.is_coinbase(),
		})
	}

	fn chain_tx_stats(&self, nblocks: Option<usize>, _blockhash: Option<String>) -> Result<ChainTxStats, Error> {
		let one_month_blocks = 30 * 24 * 60 * 60 / TARGET_SPACING_SECONDS;
		let nblocks = nblocks.unwrap_or(one_month_blocks as usize) as u32;
		let start_meta = self.storage.block_meta(nblocks.into()).ok_or(block_not_found(nblocks))?;

		// TODO enable when hashes are unified
		// let (end_hash, end_meta): (SHA256D, storage::BlockMeta) = if blockhash.is_some() {
		// 	let hash = blockhash.clone().unwrap();
		// 	let global_hash: SHA256D = SHA256D::from_reversed_str(hash.clone().as_str());
		// 	// let hash =  SHA256D::from(blockhash.unwrap().as_str());
		// 	let meta = self.storage.block_meta(BlockRef::Hash(global_hash))
		// 		.ok_or(block_not_found(blockhash.unwrap()))?;
		// 	(global_hash, meta)
		// } else {
		// 	let hash = self.storage.best_block().hash;
		// 	(hash, self.storage.block_meta(BlockRef::Hash(hash)).unwrap())
		// };

		// if end_meta.number < nblocks {
		// 	return Err(invalid_params("nblocks", "nblocks is greater than blockhash"))
		// }
		// let time_start = self.storage.block_header(nblocks.into()).unwrap().raw.time;
		// let time_end = self.storage.block_header(end_meta.number.into()).unwrap().raw.time;

		// Ok(ChainTxStats {
		// 	time: time_end as usize,
		// 	txcount: end_meta.n_chain_tx as usize,
		// 	window_final_block_hash: end_hash.to_reversed_str(),
		// 	window_block_count: (end_meta.number - start_meta.number) as usize,
		// 	window_tx_count: (end_meta.n_chain_tx - start_meta.n_chain_tx) as usize,
		// 	window_interval: (time_end - time_start) as usize,
		// 	txrate: ((end_meta.n_chain_tx - start_meta.n_chain_tx) / (time_end - time_start)) as usize,
		// })

		Ok(ChainTxStats {
			time: 0,
			txcount: start_meta.n_chain_tx as usize,
			window_final_block_hash: "".to_string(),
			window_block_count: 0,
			window_tx_count: 0,
			window_interval: 0,
			txrate: 0,
		})
	}
}

impl<T> BlockChainClient<T>
where
	T: BlockChainClientCoreApi,
{
	pub fn new(core: T) -> Self {
		BlockChainClient { core }
	}
}

impl<T> BlockChain for BlockChainClient<T>
where
	T: BlockChainClientCoreApi,
{
	fn blockchain_info(&self) -> Result<BlockchainInfo, Error> {
		Ok(BlockchainInfo {
			chain: self.core.network(),
			blocks: self.core.block_count(),
			headers: self.core.header_count(),
			bestblockhash: self.best_block_hash().ok(),
			difficulty: self.difficulty().ok(),
			mediantime: self.core.median_time(),
			verificationprogress: self.core.synchronization_progress(),
			initialblockdownload: self.core.is_synchronizing(),
			chainwork: SHA256D::default(),
			size_on_disk: self.core.size_on_disk(),
			pruned: false,
			softforks: HashMap::new(),
			warnings: "".to_string(),
		})
	}

	fn best_block_hash(&self) -> Result<SHA256D, Error> {
		Ok(self.core.best_block_hash().into())
	}

	fn block_count(&self) -> Result<u32, Error> {
		Ok(self.core.block_count())
	}

	fn block_hash(&self, height: u32) -> Result<SHA256D, Error> {
		self.core
			.block_hash(height)
			.map(|h| h.into())
			.ok_or(block_at_height_not_found(height))
	}

	fn difficulty(&self) -> Result<f64, Error> {
		Ok(self.core.difficulty())
	}

	fn block(&self, hash: SHA256D, verbose: Option<bool>) -> Result<GetBlockResponse, Error> {
		let global_hash: SHA256D = hash.clone().into();
		if verbose.unwrap_or_default() {
			self.core.verbose_block(global_hash).map(|block| GetBlockResponse::Verbose(block))
		} else {
			self.core.raw_block(global_hash).map(|block| GetBlockResponse::Raw(block))
		}
		.ok_or(block_not_found(hash))
	}

	fn transaction_out(
		&self,
		transaction_hash: SHA256D,
		out_index: u32,
		_include_mempool: Option<bool>,
	) -> Result<GetTxOutResponse, Error> {
		// TODO: include_mempool
		let transaction_hash: SHA256D = transaction_hash.into();
		self.core
			.verbose_transaction_out(OutPoint {
				hash: transaction_hash,
				index: out_index,
			})
			.map(|mut response| {
				response.bestblock = response.bestblock;
				response
			})
	}

	fn transaction_out_set_info(&self) -> Result<GetTxOutSetInfoResponse, Error> {
		let best = self.core.best_block_hash();
		self.core
			.block_meta(best)
			.map(|meta| GetTxOutSetInfoResponse {
				bestblock: best,
				height: meta.number,
				disk_size: meta.total_size,
				total_amount: meta.total_supply as f64,
				transactions: meta.n_chain_tx,
				txouts: meta.n_tx_with_utxos,
				bogosize: 0,
				hash_serialized_2: SHA256D::default(),
			})
			.ok_or(unknown())
	}

	fn chain_tx_stats(&self, nblocks: Option<usize>, blockhash: Option<String>) -> Result<ChainTxStats, Error> {
		self.core.chain_tx_stats(nblocks, blockhash)
	}
}

#[cfg(test)]
pub mod tests {
	extern crate test_data;

	use super::*;
	use crate::v1::helpers::errors::block_not_found;
	use crate::v1::traits::BlockChain;
	use crate::v1::types::Bytes;
	use crate::v1::types::ScriptType;
	use crate::v1::types::{GetTxOutResponse, TransactionOutputScript};
	use crate::v1::types::{RawBlock, VerboseBlock};
	use bitcrypto::{FromInnerHex, SHA256D};
	use chain::OutPoint;
	use db::BlockChainDatabase;
	use jsonrpc_core::Error;
	use jsonrpc_core::IoHandler;
	use network::Network;
	use primitives::bytes::Bytes as GlobalBytes;
	use std::sync::Arc;

	#[derive(Default)]
	struct SuccessBlockChainClientCore;
	#[derive(Default)]
	struct ErrorBlockChainClientCore;

	impl BlockChainClientCoreApi for SuccessBlockChainClientCore {
		fn is_synchronizing(&self) -> bool {
			false
		}

		fn synchronization_progress(&self) -> f32 {
			1.0
		}

		fn size_on_disk(&self) -> u64 {
			42000
		}

		fn network(&self) -> String {
			Network::Mainnet.to_string()
		}

		fn header_count(&self) -> u32 {
			1
		}

		fn best_block_hash(&self) -> SHA256D {
			test_data::genesis().hash()
		}

		fn block_count(&self) -> u32 {
			1
		}

		fn block_hash(&self, _height: u32) -> Option<SHA256D> {
			Some(test_data::genesis().hash())
		}

		fn difficulty(&self) -> f64 {
			1f64
		}

		fn median_time(&self) -> u32 {
			3919284
		}

		fn block_meta(&self, _hash: SHA256D) -> Option<BlockMeta> {
			Some(BlockMeta::default())
		}

		fn raw_block(&self, _hash: SHA256D) -> Option<RawBlock> {
			let b2_bytes: GlobalBytes = "010000004860eb18bf1b1620e37e9490fc8a427514416fd75159ab86688e9a8300000000d5fdcc541e25de1c7a5addedf24858b8bb665c9f36ef744ee42c316022c90f9bb0bc6649ffff001d08d2bd610101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff0704ffff001d010bffffffff0100f2052a010000004341047211a824f55b505228e4c3d5194c1fcfaa15a456abdf37f9b9d97a4040afc073dee6c89064984f03385237d92167c13e236446b417ab79a0fcae412ae3316b77ac00000000".into();
			Some(RawBlock::from(b2_bytes))
		}

		fn verbose_block(&self, _hash: SHA256D) -> Option<VerboseBlock> {
			// https://blockexplorer.com/block/000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd
			// https://blockchain.info/ru/block/000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd
			// https://webbtc.com/block/000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd.json
			Some(VerboseBlock {
				hash: SHA256D::from_inner_hex("bddd99ccfda39da1b108ce1a5d70038d0a967bacb68b6b63065f626a00000000").unwrap(),
				confirmations: 1, // h2
				size: 215,
				strippedsize: 215,
				weight: 215,
				height: Some(2),
				version: 1,
				version_hex: "1".to_owned(),
				merkleroot: SHA256D::from_inner_hex("d5fdcc541e25de1c7a5addedf24858b8bb665c9f36ef744ee42c316022c90f9b").unwrap(),
				tx: vec![SHA256D::from_inner_hex("d5fdcc541e25de1c7a5addedf24858b8bb665c9f36ef744ee42c316022c90f9b").unwrap()],
				time: 1231469744,
				mediantime: None,
				nonce: 1639830024,
				bits: 486604799,
				difficulty: 1.0,
				chainwork: U256::default(),
				previousblockhash: Some(
					SHA256D::from_inner_hex("4860eb18bf1b1620e37e9490fc8a427514416fd75159ab86688e9a8300000000").unwrap(),
				),
				nextblockhash: None,
			})
		}

		fn verbose_transaction_out(&self, _prev_out: OutPoint) -> Result<GetTxOutResponse, Error> {
			Ok(GetTxOutResponse {
				bestblock: SHA256D::from_inner_hex("0000000000000000000000000000000000000000000000000000000000000056").unwrap(),
				confirmations: 777,
				value: 100000.56,
				script: TransactionOutputScript {
					asm: "Hello, world!!!".to_owned(),
					hex: Bytes::new(vec![1, 2, 3, 4]),
					req_sigs: 777,
					script_type: ScriptType::Multisig,
					addresses: vec![
						"1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".into(),
						"1H5m1XzvHsjWX3wwU781ubctznEpNACrNC".into(),
					],
				},
				version: 33,
				coinbase: false,
			})
		}

		fn chain_tx_stats(&self, _nblocks: Option<usize>, _blockhash: Option<String>) -> Result<ChainTxStats, Error> {
			Ok(ChainTxStats::default())
		}
	}

	impl BlockChainClientCoreApi for ErrorBlockChainClientCore {
		fn header_count(&self) -> u32 {
			1
		}

		fn is_synchronizing(&self) -> bool {
			false
		}

		fn synchronization_progress(&self) -> f32 {
			1.0
		}

		fn size_on_disk(&self) -> u64 {
			42000
		}

		fn network(&self) -> String {
			Network::Mainnet.to_string()
		}

		fn best_block_hash(&self) -> SHA256D {
			test_data::genesis().hash()
		}

		fn block_count(&self) -> u32 {
			1
		}

		fn block_hash(&self, _height: u32) -> Option<SHA256D> {
			None
		}

		fn difficulty(&self) -> f64 {
			1f64
		}

		fn median_time(&self) -> u32 {
			0
		}

		fn raw_block(&self, _hash: SHA256D) -> Option<RawBlock> {
			None
		}

		fn verbose_block(&self, _hash: SHA256D) -> Option<VerboseBlock> {
			None
		}

		fn verbose_transaction_out(&self, prev_out: OutPoint) -> Result<GetTxOutResponse, Error> {
			Err(block_not_found(prev_out.hash))
		}

		fn chain_tx_stats(&self, nblocks: Option<usize>, _blockhash: Option<String>) -> Result<ChainTxStats, Error> {
			Err(block_not_found(nblocks))
		}

		fn block_meta(&self, _hash: SHA256D) -> Option<BlockMeta> {
			None
		}
	}

	#[test]
	fn blockchain_info() {
		let client = BlockChainClient::new(SuccessBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblockchaininfo",
				"params": [],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","result":{"bestblockhash":"000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f","blocks":1,"chain":"main","chainwork":"0000000000000000000000000000000000000000000000000000000000000000","difficulty":1.0,"headers":1,"initialblockdownload":false,"mediantime":3919284,"pruned":false,"size_on_disk":42000,"softforks":{},"verificationprogress":1.0,"warnings":""},"id":1}"#
		);
	}

	#[test]
	fn best_block_hash_success() {
		let client = BlockChainClient::new(SuccessBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getbestblockhash",
				"params": [],
				"id": 1
			}"#),
			)
			.unwrap();

		// direct hash is 6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000
		// but client expects reverse hash
		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","result":"000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f","id":1}"#
		);
	}

	#[test]
	fn block_count_success() {
		let client = BlockChainClient::new(SuccessBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblockcount",
				"params": [],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(&sample, r#"{"jsonrpc":"2.0","result":1,"id":1}"#);
	}

	#[test]
	fn block_hash_success() {
		let client = BlockChainClient::new(SuccessBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblockhash",
				"params": [0],
				"id": 1
			}"#),
			)
			.unwrap();

		// direct hash is 6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000
		// but client expects reverse hash
		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","result":"000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f","id":1}"#
		);
	}

	#[test]
	fn block_hash_error() {
		let client = BlockChainClient::new(ErrorBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblockhash",
				"params": [0],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","error":{"code":-32099,"message":"Block at given height is not found","data":"0"},"id":1}"#
		);
	}

	#[test]
	fn difficulty_success() {
		let client = BlockChainClient::new(SuccessBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getdifficulty",
				"params": [],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(&sample, r#"{"jsonrpc":"2.0","result":1.0,"id":1}"#);
	}

	#[test]
	fn verbose_block_contents() {
		let storage = Arc::new(BlockChainDatabase::init_test_chain(vec![
			test_data::genesis().into(),
			test_data::block_h1().into(),
			test_data::block_h2().into(),
		]));

		let core = BlockChainClientCore::new(Network::Mainnet, storage, None);

		// get info on block #1:
		// https://blockexplorer.com/block/00000000839a8e6886ab5951d76f411475428afc90947ee320161bbf18eb6048
		// https://blockchain.info/block/00000000839a8e6886ab5951d76f411475428afc90947ee320161bbf18eb6048
		// https://webbtc.com/block/00000000839a8e6886ab5951d76f411475428afc90947ee320161bbf18eb6048.json
		let verbose_block =
			core.verbose_block(SHA256D::from_inner_hex("4860eb18bf1b1620e37e9490fc8a427514416fd75159ab86688e9a8300000000").unwrap());
		assert_eq!(
			verbose_block,
			Some(VerboseBlock {
				hash: SHA256D::from_inner_hex("4860eb18bf1b1620e37e9490fc8a427514416fd75159ab86688e9a8300000000").unwrap(),
				confirmations: 2, // h1 + h2
				size: 215,
				strippedsize: 215,
				weight: 860,
				height: Some(1),
				version: 1,
				version_hex: "1".to_owned(),
				merkleroot: SHA256D::from_inner_hex("982051fd1e4ba744bbbe680e1fee14677ba1a3c3540bf7b1cdb606e857233e0e").unwrap(),
				tx: vec![SHA256D::from_inner_hex("982051fd1e4ba744bbbe680e1fee14677ba1a3c3540bf7b1cdb606e857233e0e").unwrap()],
				time: 1231469665,
				mediantime: Some(1231006505),
				nonce: 2573394689,
				bits: 486604799,
				difficulty: 1.0,
				chainwork: U256::default(),
				previousblockhash: Some(
					SHA256D::from_inner_hex("6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000").unwrap()
				),
				nextblockhash: Some(SHA256D::from_inner_hex("bddd99ccfda39da1b108ce1a5d70038d0a967bacb68b6b63065f626a00000000").unwrap()),
			})
		);

		// get info on block #2:
		// https://blockexplorer.com/block/000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd
		// https://blockchain.info/ru/block/000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd
		// https://webbtc.com/block/000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd.json
		let verbose_block =
			core.verbose_block(SHA256D::from_inner_hex("bddd99ccfda39da1b108ce1a5d70038d0a967bacb68b6b63065f626a00000000").unwrap());
		assert_eq!(
			verbose_block,
			Some(VerboseBlock {
				hash: SHA256D::from_inner_hex("bddd99ccfda39da1b108ce1a5d70038d0a967bacb68b6b63065f626a00000000").unwrap(),
				confirmations: 1, // h2
				size: 215,
				strippedsize: 215,
				weight: 860,
				height: Some(2),
				version: 1,
				version_hex: "1".to_owned(),
				merkleroot: SHA256D::from_inner_hex("d5fdcc541e25de1c7a5addedf24858b8bb665c9f36ef744ee42c316022c90f9b").unwrap(),
				tx: vec![SHA256D::from_inner_hex("d5fdcc541e25de1c7a5addedf24858b8bb665c9f36ef744ee42c316022c90f9b").unwrap()],
				time: 1231469744,
				mediantime: Some(1231469665),
				nonce: 1639830024,
				bits: 486604799,
				difficulty: 1.0,
				chainwork: U256::default(),
				previousblockhash: Some(
					SHA256D::from_inner_hex("4860eb18bf1b1620e37e9490fc8a427514416fd75159ab86688e9a8300000000").unwrap()
				),
				nextblockhash: None,
			})
		);
	}

	#[test]
	fn raw_block_success() {
		let client = BlockChainClient::new(SuccessBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let expected = r#"{"jsonrpc":"2.0","result":"010000004860eb18bf1b1620e37e9490fc8a427514416fd75159ab86688e9a8300000000d5fdcc541e25de1c7a5addedf24858b8bb665c9f36ef744ee42c316022c90f9bb0bc6649ffff001d08d2bd610101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff0704ffff001d010bffffffff0100f2052a010000004341047211a824f55b505228e4c3d5194c1fcfaa15a456abdf37f9b9d97a4040afc073dee6c89064984f03385237d92167c13e236446b417ab79a0fcae412ae3316b77ac00000000","id":1}"#;

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblock",
				"params": ["000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd", false],
				"id": 1
			}"#),
			)
			.unwrap();
		assert_eq!(&sample, expected);

		// try without optional parameter
		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblock",
				"params": ["000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd"],
				"id": 1
			}"#),
			)
			.unwrap();
		assert_eq!(&sample, expected);
	}

	#[test]
	fn raw_block_error() {
		let client = BlockChainClient::new(ErrorBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblock",
				"params": ["000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd", false],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","error":{"code":-32099,"message":"Block with given hash is not found","data":"000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd"},"id":1}"#
		);
	}

	#[test]
	fn verbose_block_success() {
		let client = BlockChainClient::new(SuccessBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblock",
				"params": ["000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd",true],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","result":{"bits":486604799,"chainwork":"0000000000000000000000000000000000000000000000000000000000000000","confirmations":1,"difficulty":1.0,"hash":"000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd","height":2,"mediantime":null,"merkleroot":"9b0fc92260312ce44e74ef369f5c66bbb85848f2eddd5a7a1cde251e54ccfdd5","nextblockhash":null,"nonce":1639830024,"previousblockhash":"00000000839a8e6886ab5951d76f411475428afc90947ee320161bbf18eb6048","size":215,"strippedsize":215,"time":1231469744,"tx":["9b0fc92260312ce44e74ef369f5c66bbb85848f2eddd5a7a1cde251e54ccfdd5"],"version":1,"versionHex":"1","weight":215},"id":1}"#
		);
	}

	#[test]
	fn verbose_block_error() {
		let client = BlockChainClient::new(ErrorBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblock",
				"params": ["000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd", true],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","error":{"code":-32099,"message":"Block with given hash is not found","data":"000000006a625f06636b8bb6ac7b960a8d03705d1ace08b1a19da3fdcc99ddbd"},"id":1}"#
		);
	}

	#[test]
	fn verbose_transaction_out_contents() {
		let storage = Arc::new(BlockChainDatabase::init_test_chain(vec![test_data::genesis().into()]));
		let core = BlockChainClientCore::new(Network::Mainnet, storage, None);

		// get info on tx from genesis block:
		// https://blockchain.info/ru/tx/4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b
		let verbose_transaction_out = core.verbose_transaction_out(OutPoint {
			hash: SHA256D::from_inner_hex("3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a").unwrap(),
			index: 0,
		});
		assert_eq!(verbose_transaction_out, Ok(GetTxOutResponse {
				bestblock: SHA256D::from_inner_hex("6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000").unwrap(),
				confirmations: 1,
				value: 50.0,
				script: TransactionOutputScript {
					asm: "OP_PUSHBYTES_65 0x04678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5f\nOP_CHECKSIG\n".to_owned(),
					hex: Bytes::from("4104678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5fac"),
					req_sigs: 1,
					script_type: ScriptType::PubKey,
					addresses: vec!["1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".into()]
				},
				version: 1,
				coinbase: true
			}));
	}

	#[test]
	fn transaction_out_success() {
		let client = BlockChainClient::new(SuccessBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "gettxout",
				"params": ["4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b", 0],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","result":{"bestblock":"5600000000000000000000000000000000000000000000000000000000000000","coinbase":false,"confirmations":777,"scriptPubKey":{"addresses":["1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa","1H5m1XzvHsjWX3wwU781ubctznEpNACrNC"],"asm":"Hello, world!!!","hex":"01020304","reqSigs":777,"type":"multisig"},"value":100000.56,"version":33},"id":1}"#
		);
	}

	#[test]
	fn transaction_out_failure() {
		let client = BlockChainClient::new(ErrorBlockChainClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "gettxout",
				"params": ["3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a", 0],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","error":{"code":-32099,"message":"Block with given hash is not found","data":"3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a"},"id":1}"#
		);
	}
}
