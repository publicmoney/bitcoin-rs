use jsonrpc_core::Error;
use jsonrpc_derive::rpc;
use v1::types::{AddNodeOperation, NetworkInfo, NodeInfo, Peer};

/// bitcoin-rs network interface
#[rpc(server)]
pub trait Network {
	/// Returns various information about the state of p2p networking.
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "id":"1", "method": "getnetworkinfo", "params": [] }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "getnetworkinfo")]
	fn network_info(&self) -> Result<NetworkInfo, Error>;
	/// Add/remove/connect to the node
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "addnode", "params": ["127.0.0.1:8888", "add"], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "addnode", "params": ["127.0.0.1:8888", "remove"], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "addnode", "params": ["127.0.0.1:8888", "onetry"], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "addnode")]
	fn add_node(&self, node: String, operation: AddNodeOperation) -> Result<(), Error>;
	/// Query added node(s) info
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "id":"1", "method": "getaddednodeinfo", "params": [true] }' -H 'content-type: application/json' http://127.0.0.1:8332/
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "id":"1", "method": "getaddednodeinfo", "params": [true, "192.168.0.201"] }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "getaddednodeinfo")]
	fn node_info(&self, dns: bool, node_addr: Option<String>) -> Result<Vec<NodeInfo>, Error>;
	/// Returns connection count
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "id":"1", "method": "getconnectioncount", "params": [] }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "getconnectioncount")]
	fn connection_count(&self) -> Result<usize, Error>;
	/// Returns data about each connected network node as a json array of objects.
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "id":"1", "method": "getpeerinfo", "params": [] }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "getpeerinfo")]
	fn peer_info(&self) -> Result<Vec<Peer>, Error>;
}
