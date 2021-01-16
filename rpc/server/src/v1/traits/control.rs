use crate::v1::types::MemoryInfo;
use jsonrpc_core::Error;
use jsonrpc_derive::rpc;

/// bitcoin-rs control interface.
#[rpc(server)]
pub trait Control {
	/// Get information about memory usage.
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "getmemoryinfo", "params": [], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "getmemoryinfo")]
	fn get_memory_info(&self) -> Result<MemoryInfo, Error>;
	/// The total uptime of the server.
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "uptime", "params": [], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "uptime")]
	fn uptime(&self) -> Result<u64, Error>;
	/// Stop the node.
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "stop", "params": [], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "stop")]
	fn stop(&self) -> Result<(), Error>;
}
