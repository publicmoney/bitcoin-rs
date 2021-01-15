use super::super::types::{BlockHeight, StorageRef};
use crate::types::AverageSpeedMeterRef;
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
	/// Highest block we have heard of from a connected peer
	best_block_from_peer: AtomicU32,
	/// Block processing speed meter
	block_speed_meter: AverageSpeedMeterRef,
}

impl SynchronizationState {
	pub fn new(storage: StorageRef, block_speed_meter: AverageSpeedMeterRef) -> Self {
		let best_storage_block_height = storage.best_block().number;
		SynchronizationState {
			is_synchronizing: AtomicBool::new(false),
			best_storage_block_height: AtomicU32::new(best_storage_block_height),
			best_block_from_peer: AtomicU32::new(0),
			block_speed_meter,
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

	pub fn update_best_peer_block(&self, height: BlockHeight) {
		if height > self.best_block_from_peer.load(Ordering::SeqCst) {
			self.best_block_from_peer.store(height, Ordering::SeqCst)
		}
	}

	pub fn synchronization_progress(&self) -> f32 {
		let (blocks_remaining, overflow) = self
			.best_block_from_peer
			.load(Ordering::SeqCst)
			.overflowing_sub(self.best_storage_block_height.load(Ordering::SeqCst));
		if overflow {
			return 0.0;
		}
		let blocks_per_second = self.block_speed_meter.speed() as f32;
		let seconds_remaining = blocks_remaining as f32 / blocks_per_second;
		let total_seconds = self.best_block_from_peer.load(Ordering::SeqCst) as f32 / blocks_per_second as f32;
		((1.0 - (seconds_remaining / total_seconds)) * 1000.0).round() / 1000.0
	}
}

impl InboundSyncConnectionState for SynchronizationState {
	fn synchronizing(&self) -> bool {
		SynchronizationState::synchronizing(self)
	}
}

#[cfg(test)]
mod tests {
	use crate::types::StorageRef;
	use crate::utils::{AverageSpeedMeter, SynchronizationState};
	use db::BlockChainDatabase;
	use std::sync::Arc;

	#[test]
	fn test_sync_progress() {
		let block_speed_meter = Arc::new(AverageSpeedMeter::with_inspect_items(10));
		let storage: StorageRef = Arc::new(BlockChainDatabase::init_test_chain(vec![test_data::genesis().into()]));
		let sync_state = SynchronizationState::new(storage, block_speed_meter.clone());

		sync_state.update_synchronizing(true);
		sync_state.update_best_storage_block_height(2);
		sync_state.update_best_peer_block(10);
		block_speed_meter.set_speed(0.5);

		assert!(sync_state.synchronizing());
		assert_eq!(sync_state.synchronization_progress(), 0.2);
	}
}
