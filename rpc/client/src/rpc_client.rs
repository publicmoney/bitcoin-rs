use jsonrpc_core_client::futures::Future;
use jsonrpc_core_client::{RpcChannel, RpcResult, TypedClient};
use rpc_server::v1::types::{MemoryInfo, NetworkInfo};

#[derive(Clone)]
pub struct RpcClient(TypedClient);

impl From<RpcChannel> for RpcClient {
	fn from(channel: RpcChannel) -> Self {
		RpcClient(channel.into())
	}
}

impl RpcClient {
	pub fn get_memory_info(&self) -> impl Future<Output = RpcResult<MemoryInfo>> {
		self.0.call_method("getmemoryinfo", "MemoryInfo", ())
	}

	pub fn get_network_info(&self) -> impl Future<Output = RpcResult<NetworkInfo>> {
		self.0.call_method("getnetworkinfo", "NetworkInfo", ())
	}

	pub fn stop(&self) -> impl Future<Output = RpcResult<()>> {
		self.0.call_method("stop", "()", ())
	}
}
