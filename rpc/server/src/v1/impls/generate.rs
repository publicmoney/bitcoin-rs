use crate::v1::helpers::errors::invalid_params;
use crate::v1::traits::Generate;
use bitcrypto::{FromStr, SHA256D};
use jsonrpc_core::Error;
use keys::AddressHash;
use tokio::time::Duration;

pub struct GenerateClient<T: GenerateClientCoreApi> {
	core: T,
}

pub trait GenerateClientCoreApi: Send + Sync + 'static {
	fn generate_to_address(&self, n_blocks: u32, address: Option<AddressHash>, max_tries: usize) -> Vec<SHA256D>;
}

pub struct GenerateClientCore {
	local_sync_node: sync::LocalNodeRef,
}

impl GenerateClientCore {
	pub fn new(local_sync_node: sync::LocalNodeRef) -> Self {
		GenerateClientCore { local_sync_node }
	}
}

impl GenerateClientCoreApi for GenerateClientCore {
	fn generate_to_address(&self, n_blocks: u32, address: Option<AddressHash>, max_tries: usize) -> Vec<SHA256D> {
		let mut block_hashes = vec![];
		for _ in 0..n_blocks {
			if let Some(hash) = self.local_sync_node.generate_block(address, max_tries) {
				block_hashes.push(hash);
			}
			std::thread::sleep(Duration::from_secs(1));
		}
		block_hashes
	}
}

impl<T> GenerateClient<T>
where
	T: GenerateClientCoreApi,
{
	pub fn new(core: T) -> Self {
		GenerateClient { core }
	}
}

impl<T> Generate for GenerateClient<T>
where
	T: GenerateClientCoreApi,
{
	fn generate(&self, n_blocks: u32, max_tries: Option<usize>) -> Result<Vec<SHA256D>, Error> {
		Ok(self.core.generate_to_address(n_blocks, None, max_tries.unwrap_or(1000000)))
	}

	fn generate_to_address(&self, n_blocks: u32, address: String, max_tries: Option<usize>) -> Result<Vec<SHA256D>, Error> {
		let address_hash = AddressHash::from_str(&address).map_err(|e| invalid_params("address", e.to_string()))?;
		Ok(self
			.core
			.generate_to_address(n_blocks, Some(address_hash), max_tries.unwrap_or(1000000)))
	}
}
