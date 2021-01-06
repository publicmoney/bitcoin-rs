use bitcrypto::SHA256D;
use jsonrpc_core::Error;
use jsonrpc_derive::rpc;

/// bitcoin-rs control interface.
#[rpc(server)]
pub trait Generate {
	/// Mine blocks immediately to an address in the wallet.
	/// Argument #1: (numeric, required) How many blocks are generated immediately.
	/// Argument #2: (numeric, optional, default=1000000) How many iterations to try.
	/// Returns: (array) Hashes of blocks generated.
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "generate", "params": [1], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "generate")]
	fn generate(&self, n_blocks: u32, max_tries: Option<usize>) -> Result<Vec<SHA256D>, Error>;
	/// Mine blocks immediately to a specified address.
	/// Argument #1: (numeric, required) How many blocks are generated immediately.
	/// Argument #2: (string, required) The address to send the newly generated bitcoin to.
	/// Argument #3: (numeric, optional, default=1000000) How many iterations to try.
	/// Returns: (array) Hashes of blocks generated.
	/// @curl-example: curl --data-binary '{"jsonrpc": "2.0", "method": "generatetoaddress", "params": [5, "16meyfSoQV6twkAAxPe51RtMVz7PGRmWna", 100000], "id":1 }' -H 'content-type: application/json' http://127.0.0.1:8332/
	#[rpc(name = "generatetoaddress")]
	fn generate_to_address(&self, n_blocks: u32, address: String, max_tries: Option<usize>) -> Result<Vec<SHA256D>, Error>;
}
