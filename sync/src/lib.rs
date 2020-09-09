extern crate bitcrypto;
extern crate byteorder;
extern crate chain;
extern crate db;
extern crate storage;
#[macro_use]
extern crate log;
extern crate bit_vec;
extern crate futures;
extern crate linked_hash_map;
extern crate message;
extern crate murmur3;
extern crate network;
extern crate p2p;
extern crate parking_lot;
extern crate primitives;
extern crate rand;
extern crate script;
extern crate serialization as ser;
extern crate time;
extern crate verification;

mod blocks_writer;
mod inbound_connection;
mod inbound_connection_factory;
mod local_node;
mod synchronization_chain;
mod synchronization_client;
mod synchronization_client_core;
mod synchronization_executor;
mod synchronization_manager;
mod synchronization_peers;
mod synchronization_peers_tasks;
mod synchronization_server;
mod synchronization_verifier;
mod types;
mod utils;

pub use types::LocalNodeRef;
pub use types::PeersRef;

use bitcrypto::SHA256D;
use message::Services;
use network::{ConsensusParams, Network};
use parking_lot::RwLock;
use std::sync::Arc;
use utils::AverageSpeedMeter;
use verification::BackwardsCompatibleChainVerifier as ChainVerifier;

/// Number of blocks to inspect when calculating average blocks speed
const BLOCKS_SPEED_BLOCKS_TO_INSPECT: usize = 512;

/// Sync errors.
#[derive(Debug, PartialEq)]
pub enum Error {
	/// Too many orphan blocks.
	TooManyOrphanBlocks,
	/// Database error.
	Database(storage::Error),
	/// Block verification error.
	Verification(String),
}

#[derive(Debug, Clone, Default)]
/// Verification parameters.
pub struct VerificationParameters {
	/// Blocks verification level.
	pub verification_level: verification::VerificationLevel,
	/// Blocks verification edge: all blocks before this are validated using verification_level.
	/// All blocks after this (inclusive) are validated using VerificationLevel::Full level.
	pub verification_edge: SHA256D,
}

/// Synchronization events listener
pub trait SyncListener: Send + 'static {
	/// Called when node switches to synchronization state
	fn synchronization_state_switched(&self, is_synchronizing: bool);
	/// Called when new best storage block is inserted
	fn best_storage_block_inserted(&self, block_hash: &SHA256D);
}

/// Create blocks writer.
pub fn create_sync_blocks_writer(
	db: storage::SharedStore,
	consensus: ConsensusParams,
	verification_params: VerificationParameters,
) -> blocks_writer::BlocksWriter {
	blocks_writer::BlocksWriter::new(db, consensus, verification_params)
}

/// Create synchronization peers
pub fn create_sync_peers() -> PeersRef {
	use synchronization_peers::PeersImpl;

	Arc::new(PeersImpl::default())
}

/// Creates local sync node for given `db`
pub fn create_local_sync_node(
	consensus: ConsensusParams,
	db: storage::SharedStore,
	peers: PeersRef,
	verification_params: VerificationParameters,
) -> LocalNodeRef {
	use local_node::LocalNode as SyncNode;
	use miner::MemoryPool;
	use synchronization_chain::Chain as SyncChain;
	use synchronization_client::SynchronizationClient;
	use synchronization_client_core::{Config as SynchronizationConfig, CoreVerificationSink, SynchronizationClientCore};
	use synchronization_executor::LocalSynchronizationTaskExecutor as SyncExecutor;
	use synchronization_server::ServerImpl;
	use synchronization_verifier::AsyncVerifier;
	use types::SynchronizationStateRef;
	use utils::SynchronizationState;

	let network = consensus.network;
	let sync_client_config = SynchronizationConfig {
		// during regtests, peer is providing us with bad blocks => we shouldn't close connection because of this
		close_connection_on_bad_block: network != Network::Regtest,
	};
	let mut memory_pool = MemoryPool::new();
	if network == Network::Regtest {
		// during regtests, peer is providing us with zero fee transactions => we shouldn't ignore these
		memory_pool.accept_zero_fee_transactions();
	}

	let block_speed_meter = Arc::new(AverageSpeedMeter::with_inspect_items(BLOCKS_SPEED_BLOCKS_TO_INSPECT));
	let memory_pool = Arc::new(RwLock::new(memory_pool));
	let sync_state = SynchronizationStateRef::new(SynchronizationState::new(db.clone(), block_speed_meter.clone()));
	let sync_chain = SyncChain::new(db.clone(), memory_pool.clone());
	peers.require_peer_services(Services::default().with_witness(true));
	let chain_verifier = Arc::new(ChainVerifier::new(db.clone(), consensus.clone()));
	let sync_executor = SyncExecutor::new(peers.clone());
	let sync_server = Arc::new(ServerImpl::new(
		peers.clone(),
		db.clone(),
		memory_pool.clone(),
		sync_executor.clone(),
	));
	let sync_client_core = SynchronizationClientCore::new(
		sync_client_config,
		sync_state.clone(),
		peers.clone(),
		sync_executor.clone(),
		sync_chain,
		chain_verifier.clone(),
		block_speed_meter,
	);
	let verifier_sink = Arc::new(CoreVerificationSink::new(sync_client_core.clone()));
	let verifier = AsyncVerifier::new(chain_verifier, db.clone(), memory_pool.clone(), verifier_sink, verification_params);
	let sync_client = SynchronizationClient::new(sync_state.clone(), sync_client_core, verifier);
	Arc::new(SyncNode::new(
		consensus,
		db,
		memory_pool,
		peers,
		sync_state,
		sync_client,
		sync_server,
	))
}

/// Create inbound synchronization connections factory for given local sync node.
pub fn create_sync_connection_factory(peers: PeersRef, local_sync_node: LocalNodeRef) -> p2p::LocalSyncNodeRef {
	use inbound_connection_factory::InboundConnectionFactory as SyncConnectionFactory;

	SyncConnectionFactory::new(peers, local_sync_node).boxed()
}
