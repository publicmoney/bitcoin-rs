use bitcrypto::SHA256D;
use jsonrpc_core_client::futures::Future;
use jsonrpc_core_client::{RpcChannel, RpcResult, TypedClient};
use keys::AddressHash;
use rpc_server::v1::types::{MemoryInfo, NetworkInfo};

#[derive(Clone)]
pub struct RpcClient(TypedClient);

impl From<RpcChannel> for RpcClient {
	fn from(channel: RpcChannel) -> Self {
		RpcClient(channel.into())
	}
}

impl RpcClient {
	pub fn block_count(&self) -> impl Future<Output = RpcResult<u32>> {
		self.0.call_method("getblockcount", "u32", ())
	}

	pub fn memory_info(&self) -> impl Future<Output = RpcResult<MemoryInfo>> {
		self.0.call_method("getmemoryinfo", "MemoryInfo", ())
	}

	pub fn network_info(&self) -> impl Future<Output = RpcResult<NetworkInfo>> {
		self.0.call_method("getnetworkinfo", "NetworkInfo", ())
	}

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

	pub fn stop(&self) -> impl Future<Output = RpcResult<()>> {
		self.0.call_method("stop", "()", ())
	}
}
