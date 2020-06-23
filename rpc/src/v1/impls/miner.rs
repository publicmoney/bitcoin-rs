use crate::v1::traits::Miner;
use crate::v1::types::{BlockTemplate, BlockTemplateRequest, MempoolInfo};
use jsonrpc_core::Error;
use miner;
use miner::MemoryPoolInformation;
use sync;

pub struct MinerClient<T: MinerClientCoreApi> {
	core: T,
}

pub trait MinerClientCoreApi: Send + Sync + 'static {
	fn get_block_template(&self) -> miner::BlockTemplate;
	fn get_mempool_info(&self) -> miner::MemoryPoolInformation;
}

pub struct MinerClientCore {
	local_sync_node: sync::LocalNodeRef,
}

impl MinerClientCore {
	pub fn new(local_sync_node: sync::LocalNodeRef) -> Self {
		MinerClientCore { local_sync_node }
	}
}

impl MinerClientCoreApi for MinerClientCore {
	fn get_block_template(&self) -> miner::BlockTemplate {
		self.local_sync_node.get_block_template()
	}

	fn get_mempool_info(&self) -> MemoryPoolInformation {
		self.local_sync_node.information().chain.transactions
	}
}

impl<T> MinerClient<T>
where
	T: MinerClientCoreApi,
{
	pub fn new(core: T) -> Self {
		MinerClient { core }
	}
}

impl<T> Miner for MinerClient<T>
where
	T: MinerClientCoreApi,
{
	fn get_block_template(&self, _request: BlockTemplateRequest) -> Result<BlockTemplate, Error> {
		Ok(self.core.get_block_template().into())
	}

	fn mempool_info(&self) -> Result<MempoolInfo, Error> {
		Ok(self.core.get_mempool_info().into())
	}
}

#[cfg(test)]
pub mod tests {
	use super::*;
	use crate::v1::traits::Miner;
	use chain;
	use jsonrpc_core::IoHandler;
	use miner;
	use primitives::hash::H256;

	#[derive(Default)]
	struct SuccessMinerClientCore;

	impl MinerClientCoreApi for SuccessMinerClientCore {
		fn get_block_template(&self) -> miner::BlockTemplate {
			let tx: chain::Transaction =
				"00000000013ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a0000000000000000000101000000000000000000000000"
					.into();
			miner::BlockTemplate {
				version: 777,
				previous_header_hash: H256::from(1),
				time: 33,
				bits: 44.into(),
				height: 55,
				transactions: vec![tx.into()],
				coinbase_value: 66,
				size_limit: 77,
				sigop_limit: 88,
			}
		}

		fn get_mempool_info(&self) -> MemoryPoolInformation {
			MemoryPoolInformation {
				transactions_count: 500,
				transactions_size_in_bytes: 50000,
			}
		}
	}

	#[test]
	fn getblocktemplate_accepted() {
		let client = MinerClient::new(SuccessMinerClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getblocktemplate",
				"params": [{}],
				"id": 1
			}"#),
			)
			.unwrap();

		// direct hash is 0100000000000000000000000000000000000000000000000000000000000000
		// but client expects reverse hash
		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","result":{"bits":44,"coinbaseaux":null,"coinbasetxn":null,"coinbasevalue":66,"curtime":33,"height":55,"mintime":null,"mutable":null,"noncerange":null,"previousblockhash":"0000000000000000000000000000000000000000000000000000000000000001","rules":null,"sigoplimit":88,"sizelimit":77,"target":"0000000000000000000000000000000000000000000000000000000000000000","transactions":[{"data":"00000000013ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a0000000000000000000101000000000000000000000000","depends":null,"fee":null,"hash":null,"required":false,"sigops":null,"txid":null,"weight":null}],"vbavailable":null,"vbrequired":null,"version":777,"weightlimit":null},"id":1}"#
		);
	}

	#[test]
	fn getmempool_accepted() {
		let client = MinerClient::new(SuccessMinerClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getmempoolinfo",
				"params": [],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(
			&sample,
			r#"{"jsonrpc":"2.0","result":{"bytes":0,"maxmempool":0,"mempoolminfee":0,"minrelaytxfee":0,"size":500,"usage":50000},"id":1}"#
		);
	}
}
