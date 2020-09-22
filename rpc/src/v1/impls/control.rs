use crate::v1::traits::Control;
use crate::v1::types::MemoryInfo;
use jsonrpc_core::Error;
use memory::Memory;
use std::sync::Arc;

pub struct ControlClient<T: ControlClientCoreApi> {
	core: T,
}

pub trait ControlClientCoreApi: Send + Sync + 'static {
	fn get_memory_info(&self) -> MemoryInfo;
}

pub struct ControlClientCore {
	memory: Arc<Memory>,
}

impl ControlClientCore {
	pub fn new(memory: Arc<Memory>) -> Self {
		ControlClientCore { memory }
	}
}

impl ControlClientCoreApi for ControlClientCore {
	fn get_memory_info(&self) -> MemoryInfo {
		let stats = self.memory.get_memory_usage();

		MemoryInfo {
			used: stats.active,
			free: stats.resident - stats.active,
			total: stats.resident,
		}
	}
}

impl<T> ControlClient<T>
where
	T: ControlClientCoreApi,
{
	pub fn new(core: T) -> Self {
		ControlClient { core }
	}
}

impl<T> Control for ControlClient<T>
where
	T: ControlClientCoreApi,
{
	fn get_memory_info(&self) -> Result<MemoryInfo, Error> {
		Ok(self.core.get_memory_info().into())
	}
}

#[cfg(test)]
pub mod tests {
	use super::*;
	use jsonrpc_core::IoHandler;

	#[derive(Default)]
	struct SuccessControlClientCore;

	impl ControlClientCoreApi for SuccessControlClientCore {
		fn get_memory_info(&self) -> MemoryInfo {
			MemoryInfo {
				used: 0,
				free: 0,
				total: 0,
			}
		}
	}

	#[test]
	fn test_get_memory_info() {
		let client = ControlClient::new(SuccessControlClientCore::default());
		let mut handler = IoHandler::new();
		handler.extend_with(client.to_delegate());

		let sample = handler
			.handle_request_sync(
				&(r#"
			{
				"jsonrpc": "2.0",
				"method": "getmemoryinfo",
				"params": [],
				"id": 1
			}"#),
			)
			.unwrap();

		assert_eq!(&sample, r#"{"jsonrpc":"2.0","result":{"free":0,"total":0,"used":0},"id":1}"#);
	}
}
