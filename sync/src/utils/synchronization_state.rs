use super::super::types::{BlockHeight, StorageRef};
use p2p::InboundSyncConnectionState;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Shared synchronization client state.
/// It can be slightly inaccurate, but that's ok for the uses.
#[derive(Debug)]
pub struct SynchronizationState {
	/// Is synchronization in progress?
	is_synchronizing: AtomicBool,
	/// Height of best block in the storage
	best_storage_block_height: AtomicU32,
}

impl SynchronizationState {
	pub fn with_storage(storage: StorageRef) -> Self {
		let best_storage_block_height = storage.best_block().number;
		SynchronizationState {
			is_synchronizing: AtomicBool::new(false),
			best_storage_block_height: AtomicU32::new(best_storage_block_height),
		}
	}

	pub fn synchronizing(&self) -> bool {
		self.is_synchronizing.load(Ordering::SeqCst)
	}

	pub fn update_synchronizing(&self, synchronizing: bool) {
		self.is_synchronizing.store(synchronizing, Ordering::SeqCst);
	}

	pub fn best_storage_block_height(&self) -> BlockHeight {
		self.best_storage_block_height.load(Ordering::SeqCst) as BlockHeight
	}

	pub fn update_best_storage_block_height(&self, height: BlockHeight) {
		self.best_storage_block_height.store(height, Ordering::SeqCst);
	}
}

impl InboundSyncConnectionState for SynchronizationState {
	fn synchronizing(&self) -> bool {
		SynchronizationState::synchronizing(self)
	}
}
