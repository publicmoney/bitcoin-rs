use crate::v1::traits::Control;
use crate::v1::types::MemoryInfo;
use jsonrpc_core::Error;
use memory::Memory;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

pub struct ControlClient<T: ControlClientCoreApi> {
	core: T,
}

pub trait ControlClientCoreApi: Send + Sync + 'static {
	fn get_memory_info(&self) -> MemoryInfo;
	fn uptime(&self) -> Duration;
	fn stop(&self);
}

pub struct ControlClientCore {
	memory: Arc<Memory>,
	shutdown_signal: Arc<Notify>,
}

impl ControlClientCore {
	pub fn new(memory: Arc<Memory>, shutdown_signal: Arc<Notify>) -> Self {
		ControlClientCore { memory, shutdown_signal }
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

	fn uptime(&self) -> Duration {
		self.memory.uptime()
	}

	fn stop(&self) {
		self.shutdown_signal.notify_one();
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

	fn uptime(&self) -> Result<u64, Error> {
		Ok(self.core.uptime().as_secs())
	}

	fn stop(&self) -> Result<(), Error> {
		Ok(self.core.stop())
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

		fn uptime(&self) -> Duration {
			unimplemented!()
		}

		fn stop(&self) {
			unimplemented!()
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
