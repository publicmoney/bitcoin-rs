use jsonrpc_core::Error;
use jsonrpc_derive::rpc;
use v1::types::{BlockTemplate, BlockTemplateRequest, MempoolInfo};

/// bitcoin-rs miner data interface.
#[rpc(server)]
pub trait Miner {
	/// Get block template for mining.
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "getblocktemplate", "params": [{"capabilities": ["coinbasetxn", "workid", "coinbase/append"]}], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "getblocktemplate")]
	fn get_block_template(&self, request: BlockTemplateRequest) -> Result<BlockTemplate, Error>;
	/// Get memory pool info
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "getmempoolinfo", "params": [], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "getmempoolinfo")]
	fn mempool_info(&self) -> Result<MempoolInfo, Error>;
}
