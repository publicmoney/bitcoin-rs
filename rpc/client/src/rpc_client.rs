use bitcrypto::SHA256D;
use jsonrpc_core_client::futures::Future;
use jsonrpc_core_client::{RpcChannel, RpcResult, TypedClient};
use keys::AddressHash;
use rpc_server::v1::types::{
	AddNodeOperation, BlockchainInfo, ChainTxStats, GetBlockResponse, GetTxOutResponse, GetTxOutSetInfoResponse, MemoryInfo, NetworkInfo,
	NodeInfo, Peer,
};
use std::net::{IpAddr, SocketAddr};

#[derive(Clone)]
pub struct RpcClient(TypedClient);

impl From<RpcChannel> for RpcClient {
	fn from(channel: RpcChannel) -> Self {
		RpcClient(channel.into())
	}
}

impl RpcClient {
	/// Blockchain
	pub fn blockchain_info(&self) -> impl Future<Output = RpcResult<BlockchainInfo>> {
		self.0.call_method("getblockchaininfo", "BlockchainInfo", ())
	}

	pub fn best_block_hash(&self) -> impl Future<Output = RpcResult<SHA256D>> {
		self.0.call_method("getbestblockhash", "SHA256D", ())
	}

	pub fn block_count(&self) -> impl Future<Output = RpcResult<u32>> {
		self.0.call_method("getblockcount", "u32", ())
	}

	pub fn block_hash(&self, height: u32) -> impl Future<Output = RpcResult<SHA256D>> {
		self.0.call_method("getblockhash", "SHA256D", (height,))
	}

	pub fn difficulty(&self) -> impl Future<Output = RpcResult<f64>> {
		self.0.call_method("getdifficulty", "f64", ())
	}

	pub fn block(&self, hash: SHA256D, verbose: Option<bool>) -> impl Future<Output = RpcResult<GetBlockResponse>> {
		self.0.call_method("getblock", "GetBlockResponse", (hash, verbose))
	}

	pub fn transaction_out(
		&self,
		hash: SHA256D,
		out_index: u32,
		include_mempool: Option<bool>,
	) -> impl Future<Output = RpcResult<GetTxOutResponse>> {
		self.0
			.call_method("gettxout", "GetTxOutResponse", (hash, out_index, include_mempool))
	}

	pub fn transaction_out_set_info(&self) -> impl Future<Output = RpcResult<GetTxOutSetInfoResponse>> {
		self.0.call_method("gettxoutsetinfo", "GetTxOutSetInfoResponse", ())
	}

	pub fn chain_tx_stats(&self, nblocks: Option<usize>, blockhash: Option<String>) -> impl Future<Output = RpcResult<ChainTxStats>> {
		self.0.call_method("getchaintxstats", "ChainTxStats", (nblocks, blockhash))
	}

	/// Control
	pub fn memory_info(&self) -> impl Future<Output = RpcResult<MemoryInfo>> {
		self.0.call_method("getmemoryinfo", "MemoryInfo", ())
	}

	pub fn uptime(&self) -> impl Future<Output = RpcResult<u64>> {
		self.0.call_method("uptime", "u64", ())
	}

	pub fn stop(&self) -> impl Future<Output = RpcResult<()>> {
		self.0.call_method("stop", "()", ())
	}

	/// Generate
	pub fn generate(&self, n_blocks: u32, max_tries: Option<usize>) -> impl Future<Output = RpcResult<Vec<SHA256D>>> {
		self.0.call_method("generate", "Vec<SHA256D>", (n_blocks, max_tries))
	}

	pub fn generate_to_address(
		&self,
		n_blocks: u32,
		address: AddressHash,
		max_tries: Option<usize>,
	) -> impl Future<Output = RpcResult<Vec<SHA256D>>> {
		self.0
			.call_method("generatetoaddress", "Vec<SHA256D>", (n_blocks, address, max_tries))
	}

	/// Network
	pub fn network_info(&self) -> impl Future<Output = RpcResult<NetworkInfo>> {
		self.0.call_method("getnetworkinfo", "NetworkInfo", ())
	}

	pub fn add_node(&self, node: SocketAddr, operation: AddNodeOperation) -> impl Future<Output = RpcResult<()>> {
		self.0.call_method("addnode", "()", (node, operation))
	}

	pub fn node_info(&self, dns: bool, node_addr: Option<IpAddr>) -> impl Future<Output = RpcResult<Vec<NodeInfo>>> {
		self.0.call_method("getaddednodeinfo", "Vec<NodeInfo>", (dns, node_addr))
	}

	pub fn connection_count(&self) -> impl Future<Output = RpcResult<usize>> {
		self.0.call_method("getconnectioncount", "usize", ())
	}

	pub fn peer_info(&self) -> impl Future<Output = RpcResult<Vec<Peer>>> {
		self.0.call_method("getpeerinfo", "Vec<Peer>", ())
	}
}
