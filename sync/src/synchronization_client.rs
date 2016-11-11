use std::thread;
use std::sync::Arc;
use std::cmp::{min, max};
use std::collections::{HashMap, HashSet, VecDeque};
use std::collections::hash_map::Entry;
use std::sync::mpsc::{channel, Sender, Receiver};
use parking_lot::{Mutex, Condvar};
use futures::{BoxFuture, Future, finished};
use futures::stream::Stream;
use tokio_core::reactor::{Handle, Interval};
use futures_cpupool::CpuPool;
use linked_hash_map::LinkedHashMap;
use db;
use chain::{Block, BlockHeader, RepresentH256};
use primitives::hash::H256;
use synchronization_peers::Peers;
#[cfg(test)] use synchronization_peers::{Information as PeersInformation};
use synchronization_chain::{Chain, ChainRef, BlockState, HeadersIntersection};
#[cfg(test)]
use synchronization_chain::{Information as ChainInformation};
use verification::{ChainVerifier, Error as VerificationError, Verify};
use synchronization_executor::{Task, TaskExecutor};
use synchronization_manager::{manage_synchronization_peers, manage_unknown_orphaned_blocks, MANAGEMENT_INTERVAL_MS};
use hash_queue::HashPosition;
use time;
use std::time::Duration;

#[cfg_attr(feature="cargo-clippy", allow(doc_markdown))]
///! TODO: update with headers-first corrections
///!
///! Blocks synchronization process:
///!
///! When new peer is connected:
///! 1) send `getheaders` message with full block locator hashes (see `LocalNode`)
///!
///! on_new_blocks_headers: When `headers` message is received from peer:
///! 1) queue_intersection = intersect(queue, inventory)
///! 2) if !queue_intersection.is_empty(): ===> responded with blocks within sync window
///! 2.1) remember peer as useful
///! 2.2) inventory_rest = inventory - queue_intersection
///! 2.3) if inventory_rest.is_empty(): ===> no new unknown blocks in inventory
///! 2.3.1) stop (2.3)
///! 2.4) if !inventory_rest.is_empty(): ===> has new unknown blocks in inventory
///! 2.4.1) queue_rest = queue after intersection
///! 2.4.2) if queue_rest.is_empty(): ===> has new unknown blocks in inventory, no fork
///! 2.4.2.1) scheduled_blocks.append(inventory_rest)
///! 2.4.2.2) stop (2.4.2)
///! 2.4.3) if !queue_rest.is_empty(): ===> has new unknown blocks in inventory, fork
///! 2.4.3.1) scheduled_blocks.append(inventory_rest)
///! 2.4.3.2) stop (2.4.3)
///! 2.4.3) stop (2.4)
///! 2.5) stop (2)
///! 3) if queue_intersection.is_empty(): ===> responded with out-of-sync-window blocks
///! 3.1) last_known_block = inventory.last(b => b.is_known())
///! 3.2) if last_known_block == None: ===> we know nothing about these blocks & we haven't asked for these
///! 3.2.1) if !synchronizing => remember peer as useful + ask for blocks
///! 3.2.1) if synchronizing => peer will be excluded later by management thread
///! 3.2.2) stop (3.2)
///! 3.3) if last_known_block == last(inventory): ===> responded with all-known-blocks
///! 3.3.1) if syncing, remember peer as useful (possibly had failures before && have been excluded from sync)
///! 3.3.2) stop (3.3)
///! 3.4) if last_known_block in the middle of inventory: ===> responded with forked blocks
///! 3.4.1) remember peer as useful
///! 3.4.2) inventory_rest = inventory after last_known_block
///! 3.4.3) scheduled_blocks.append(inventory_rest)
///! 3.4.4) stop (3.4)
///! 3.5) stop (3)
///!
///! on_peer_block: After receiving `block` message:
///! 1) if block_state(block) in (Verifying, Stored): ===> late delivery
///! 1.1) remember peer as useful
///! 1.2) stop (1)
///! 2) if block_state(block) in (Scheduled, Requested): ===> future/on-time delivery
///! 2.1) remember peer as useful
///! 2.2) if block_state(block.parent) in (Verifying, Stored): ===> we can proceed with verification
///! 2.2.1) remove block from current queue (Verifying || Stored)
///! 2.2.2) append block to the verification queue
///! 2.2.3) queue verification().and_then(on_block_verification_success).or_else(on_block_verification_error)
///! 2.2.4) try to verify orphan blocks
///! 2.2.5) stop (2.2)
///! 2.3) if block_state(block.parent) in (Requested, Scheduled): ===> we have found an orphan block
///! 2.3.1) remove block from current queue (Verifying || Stored)
///! 2.3.2) append block to the orphans
///! 2.3.3) stop (2.3)
///! 2.4) if block_state(block.parent) == Unknown: ===> bad block found
///! 2.4.1) remove block from current queue (Verifying || Stored)
///! 2.4.2) stop (2.4)
///! 2.5) stop (2)
///! 3) if block_state(block) == Unknown: ===> maybe we are on-top of chain && new block is announced?
///! 3.1) if block_state(block.parent_hash) == Unknown: ===> we do not know parent
///! 3.1.1) ignore this block
///! 3.1.2) stop (3.1)
///! 3.2) if block_state(block.parent_hash) in (Verifying, Stored): ===> fork found, can verify
///! 3.2.1) ask peer for best inventory (after this block)
///! 3.2.2) append block to verifying queue
///! 3.2.3) queue verification().and_then(on_block_verification_success).or_else(on_block_verification_error)
///! 3.2.4) stop (3.2)
///! 3.3) if block_state(block.parent_hash) in (Requested, Scheduled): ===> fork found, add as orphan
///! 3.3.1) ask peer for best inventory (after this block)
///! 3.3.2) append block to orphan
///! 3.3.3) stop (3.3)
///! 3.4) stop (2)
///! + if no blocks left in scheduled + requested queue => we are saturated => ask all peers for inventory & forget
///!
///! execute_synchronization_tasks: After receiving `headers`/`inventory` message OR receiving `block` message OR when management thread schedules tasks:
///! 1) if there are blocks in `scheduled` queue AND we can fit more blocks into memory: ===> ask for blocks
///! 1.1) select idle peers
///! 1.2) for each idle peer: query chunk of blocks from `scheduled` queue
///! 1.3) move requested blocks from `scheduled` to `requested` queue
///! 1.4) mark idle peers as active
///! 1.5) stop (1)
///! 2) if `scheduled` queue is not yet saturated: ===> ask for new blocks hashes
///! 2.1) for each idle peer: send shortened `getblocks` message
///! 2.2) 'forget' idle peers => they will be added again if respond with inventory
///! 2.3) stop (2)
///!
///! manage_synchronization_peers: When management thread awakes:
///! 1) for peer in active_peers.where(p => now() - p.last_request_time() > failure_interval):
///! 1.1) return all peer' tasks to the tasks pool + TODO: filter tasks (if we have requested some hash several times from several peers && they haven't responded => drop this hash + reset sync???)
///! 1.2) increase # of failures for this peer
///! 1.3) if # of failures > max_failures: ===> super-bad peer
///! 1.3.1) forget peer
///! 1.3.3) stop (1.3)
///! 1.4) if # of failures <= max_failures: ===> bad peer
///! 1.4.1) move peer to idle pool
///! 1.4.2) stop (1.4)
///! 2) schedule tasks from pool (if any)
///!
///! on_block_verification_success: When verification completes scuccessfully:
///! 1) if block_state(block) != Verifying: ===> parent verification failed
///! 1.1) stop (1)
///! 2) remove from verifying queue
///! 3) insert to the db
///!
///! on_block_verification_error: When verification completes with an error:
///! 1) remove block from verification queue
///! 2) remove all known children from all queues [so that new `block` messages will be ignored in on_peer_block.3.1.1]
///!

/// Approximate maximal number of blocks hashes in scheduled queue.
const MAX_SCHEDULED_HASHES: u32 = 4 * 1024;
/// Approximate maximal number of blocks hashes in requested queue.
const MAX_REQUESTED_BLOCKS: u32 = 256;
/// Approximate maximal number of blocks in verifying queue.
const MAX_VERIFYING_BLOCKS: u32 = 256;
/// Minimum number of blocks to request from peer
const MIN_BLOCKS_IN_REQUEST: u32 = 32;
/// Maximum number of blocks to request from peer
const MAX_BLOCKS_IN_REQUEST: u32 = 128;

/// Synchronization state
#[derive(Debug, Clone, Copy)]
pub enum State {
	/// We know that there are > 1 unknown blocks, unknown to us in the blockchain
	Synchronizing(f64, u32),
	/// There is only one unknown block in the blockchain
	NearlySaturated,
	/// We have downloaded all blocks of the blockchain of which we have ever heard
	Saturated,
}

/// Information on current synchronization state.
#[cfg(test)]
#[derive(Debug)]
pub struct Information {
	/// Current synchronization state.
	pub state: State,
	/// Information on synchronization peers.
	pub peers: PeersInformation,
	/// Current synchronization chain inormation.
	pub chain: ChainInformation,
	/// Number of currently orphaned blocks.
	pub orphaned: usize,
}

/// Verification thread tasks
enum VerificationTask {
	/// Verify single block
	VerifyBlock(Block),
	/// Stop verification thread
	Stop,
}

/// Synchronization client trait
pub trait Client : Send + 'static {
	fn best_block(&self) -> db::BestBlock;
	fn on_new_blocks_inventory(&mut self, peer_index: usize, peer_hashes: Vec<H256>);
	fn on_new_blocks_headers(&mut self, peer_index: usize, blocks_headers: Vec<BlockHeader>);
	fn on_peer_block(&mut self, peer_index: usize, block: Block);
	fn on_peer_disconnected(&mut self, peer_index: usize);
	fn get_peers_nearly_blocks_waiter(&mut self, peer_index: usize) -> (bool, Option<Arc<PeersBlocksWaiter>>);
	fn on_block_verification_success(&mut self, block: Block);
	fn on_block_verification_error(&mut self, err: &VerificationError, hash: &H256);
}

/// Synchronization peer blocks waiter
#[derive(Default)]
pub struct PeersBlocksWaiter {
	/// Awake mutex
	peer_blocks_lock: Mutex<bool>,
	/// Awake event
	peer_blocks_done: Condvar,
}

/// Synchronization client configuration options.
pub struct Config {
	/// Number of threads to allocate in synchronization CpuPool.
	pub threads_num: usize,
	/// Do not verify incoming blocks before inserting to db.
	pub skip_verification: bool,
}

/// Synchronization client.
pub struct SynchronizationClient<T: TaskExecutor> {
	/// Synchronization state.
	state: State,
	/// Cpu pool.
	pool: CpuPool,
	/// Sync management worker.
	management_worker: Option<BoxFuture<(), ()>>,
	/// Synchronization peers.
	peers: Peers,
	/// Task executor.
	executor: Arc<Mutex<T>>,
	/// Chain reference.
	chain: ChainRef,
	/// Blocks from requested_hashes, but received out-of-order.
	orphaned_blocks: HashMap<H256, HashMap<H256, Block>>,
	/// Blocks that we have received without requesting with receiving time.
	unknown_blocks: LinkedHashMap<H256, f64>,
	/// Verification work transmission channel.
	verification_work_sender: Option<Sender<VerificationTask>>,
	/// Verification thread.
	verification_worker_thread: Option<thread::JoinHandle<()>>,
	/// Verifying blocks by peer
	verifying_blocks_by_peer: HashMap<H256, usize>,
	/// Verifying blocks waiters
	verifying_blocks_waiters: HashMap<usize, (HashSet<H256>, Option<Arc<PeersBlocksWaiter>>)>,
}

impl Default for Config {
	fn default() -> Self {
		Config {
			threads_num: 4,
			skip_verification: false,
		}
	}
}

impl State {
	pub fn is_saturated(&self) -> bool {
		match self {
			&State::Saturated => true,
			_ => false,
		}
	}

	pub fn is_synchronizing(&self) -> bool {
		match *self {
			State::Synchronizing(_, _) => true,
			_ => false,
		}
	}

	pub fn is_nearly_saturated(&self) -> bool {
		match self {
			&State::NearlySaturated => true,
			_ => false,
		}
	}
}

impl<T> Drop for SynchronizationClient<T> where T: TaskExecutor {
	fn drop(&mut self) {
		if let Some(join_handle) = self.verification_worker_thread.take() {
			// ignore send error here <= destructing anyway
			let _ = self.verification_work_sender
				.take()
				.expect("Some(join_handle) => Some(verification_work_sender)")
				.send(VerificationTask::Stop);
			join_handle.join().expect("Clean shutdown.");
		}
	}
}

impl<T> Client for SynchronizationClient<T> where T: TaskExecutor {
	/// Get best known block
	fn best_block(&self) -> db::BestBlock {
		self.chain.read().best_block()
	}

	/// Try to queue synchronization of unknown blocks when new inventory is received.
	fn on_new_blocks_inventory(&mut self, peer_index: usize, blocks_hashes: Vec<H256>) {
		// we use headers-first synchronization
		// we know nothing about these blocks
		// =>

		// if we are in synchronization state, we will ignore this message
		if self.state.is_synchronizing() {
			return;
		}

		// else => request all unknown blocks
		let unknown_blocks_hashes: Vec<_> = {
			let chain = self.chain.read();
			blocks_hashes.into_iter()
				.filter(|h| chain.block_state(&h) == BlockState::Unknown)
				.filter(|h| !self.unknown_blocks.contains_key(h))
				.collect()
		};

		let mut executor = self.executor.lock();
		executor.execute(Task::RequestBlocks(peer_index, unknown_blocks_hashes))
	}

	/// Try to queue synchronization of unknown blocks when blocks headers are received.
	fn on_new_blocks_headers(&mut self, peer_index: usize, blocks_headers: Vec<BlockHeader>) {
		let blocks_hashes = {
			// we can't process headers message if it has no link to our headers
			let ref header0 = blocks_headers[0];
			if {
				self.chain.read().block_state(&header0.previous_header_hash) == BlockState::Unknown
			} {
				warn!(target: "sync", "Previous header of the first header from peer#{} `headers` message is unknown. First: {:?}. Previous: {:?}", peer_index, header0.hash(), header0.previous_header_hash);
				return;
			}

			// TODO: add full blocks headers validation here
			// validate blocks headers before scheduling
			let mut blocks_hashes: Vec<H256> = Vec::with_capacity(blocks_headers.len());
			let mut prev_block_hash = header0.previous_header_hash.clone();
			for block_header in blocks_headers.iter() {
				let block_header_hash = block_header.hash();
				if block_header.previous_header_hash != prev_block_hash {
					warn!(target: "sync", "Neighbour headers in peer#{} `headers` message are unlinked: Prev: {:?}, PrevLink: {:?}, Curr: {:?}", peer_index, prev_block_hash, block_header.previous_header_hash, block_header_hash);
					return;
				}

				blocks_hashes.push(block_header_hash.clone());
				prev_block_hash = block_header_hash;
			}

			blocks_hashes
		};

		// now insert unknown blocks to the queue
		self.process_new_blocks_headers(peer_index, blocks_hashes, blocks_headers);
		self.execute_synchronization_tasks(None);
	}

	/// Process new block.
	fn on_peer_block(&mut self, peer_index: usize, block: Block) {
		let block_hash = block.hash();

		// update peers to select next tasks
		self.peers.on_block_received(peer_index, &block_hash);

		self.process_peer_block(peer_index, block_hash, block);
		self.execute_synchronization_tasks(None);
	}

	/// Peer disconnected.
	fn on_peer_disconnected(&mut self, peer_index: usize) {
		// when last peer is disconnected, reset, but let verifying blocks be verified
		if self.peers.on_peer_disconnected(peer_index) {
			self.switch_to_saturated_state(false);
		}
	}

	/// Get waiter to wait until peer blocks are processed in nearly synchronized state
	fn get_peers_nearly_blocks_waiter(&mut self, peer_index: usize) -> (bool, Option<Arc<PeersBlocksWaiter>>) {
		// if we are currently synchronizing => no need to wait
		if self.state.is_synchronizing() {
			return (false, None);
		}

		// we have to wait until all previous peer requests are server
		match self.verifying_blocks_waiters.entry(peer_index) {
			Entry::Occupied(mut entry) => {
				if entry.get().1.is_none() {
					entry.get_mut().1 = Some(Arc::new(PeersBlocksWaiter::default()));
				}
				// also wait until all blocks, supplied by this peer are verified
				(true, entry.get().1.clone())
			},
			_ => (true, None),
		}
	}

	/// Process successful block verification
	fn on_block_verification_success(&mut self, block: Block) {
		let hash = block.hash();
		// insert block to the storage
		{
			let mut chain = self.chain.write();

			// remove block from verification queue
			// header is removed in `insert_best_block` call
			// or it is removed earlier, when block was removed from the verifying queue
			if chain.forget_with_state_leave_header(&hash, BlockState::Verifying) != HashPosition::Missing {
				// block was in verification queue => insert to storage
				chain.insert_best_block(hash.clone(), block)
					.expect("Error inserting to db.");
			}
		}

		// awake threads, waiting for this block insertion
		self.awake_waiting_threads(&hash);

		// continue with synchronization
		self.execute_synchronization_tasks(None);
	}

	/// Process failed block verification
	fn on_block_verification_error(&mut self, err: &VerificationError, hash: &H256) {
		warn!(target: "sync", "Block {:?} verification failed with error {:?}", hash, err);

		{
			let mut chain = self.chain.write();

			// forget for this block and all its children
			// headers are also removed as they all are invalid
			chain.forget_with_children(&hash);
		}

		// awake threads, waiting for this block insertion
		self.awake_waiting_threads(&hash);

		// start new tasks
		self.execute_synchronization_tasks(None);
	}
}

impl<T> SynchronizationClient<T> where T: TaskExecutor {
	/// Create new synchronization window
	pub fn new(config: Config, handle: &Handle, executor: Arc<Mutex<T>>, chain: ChainRef) -> Arc<Mutex<Self>> {
		let sync = Arc::new(Mutex::new(
			SynchronizationClient {
				state: State::Saturated,
				peers: Peers::new(),
				pool: CpuPool::new(config.threads_num),
				management_worker: None,
				executor: executor,
				chain: chain.clone(),
				orphaned_blocks: HashMap::new(),
				unknown_blocks: LinkedHashMap::new(),
				verification_work_sender: None,
				verification_worker_thread: None,
				verifying_blocks_by_peer: HashMap::new(),
				verifying_blocks_waiters: HashMap::new(),
			}
		));

		if !config.skip_verification {
			let (verification_work_sender, verification_work_receiver) = channel();
			let csync = sync.clone();
			let mut lsync = sync.lock();
			let storage = chain.read().storage();
			lsync.verification_work_sender = Some(verification_work_sender);
			lsync.verification_worker_thread = Some(thread::Builder::new()
				.name("Sync verification thread".to_string())
				.spawn(move || {
					SynchronizationClient::verification_worker_proc(csync, storage, verification_work_receiver)
				})
				.expect("Error creating verification thread"));
		}

		// TODO: start management worker only when synchronization is started
		//       currently impossible because there is no way to call Interval::new with Remote && Handle is not-Send
		{
			let csync = Arc::downgrade(&sync);
			let mut sync = sync.lock();
			let management_worker = Interval::new(Duration::from_millis(MANAGEMENT_INTERVAL_MS), handle)
				.expect("Failed to create interval")
				.and_then(move |_| {
					let client = match csync.upgrade() {
						Some(client) => client,
						None => return Ok(()),
					};
					let mut client = client.lock();
					if client.state.is_synchronizing() || client.state.is_nearly_saturated() {
						let blocks_to_request = manage_synchronization_peers(&mut client.peers);
						// if no peers left => we are saturated
						if !client.peers.any() {
							client.switch_to_saturated_state(false);
						} else {
							client.execute_synchronization_tasks(blocks_to_request);
						}

						if let Some(orphans_to_remove) = manage_unknown_orphaned_blocks(&mut client.unknown_blocks) {
							client.remove_orphaned_blocks(orphans_to_remove.into_iter().collect());
						}
					}
					Ok(())
				})
				.for_each(|_| Ok(()))
				.then(|_| finished::<(), ()>(()))
				.boxed();
			sync.management_worker = Some(sync.pool.spawn(management_worker).boxed());
		}

		sync
	}

	/// Get information on current synchronization state.
	#[cfg(test)]
	pub fn information(&self) -> Information {
		Information {
			state: self.state,
			peers: self.peers.information(),
			chain: self.chain.read().information(),
			orphaned: self.orphaned_blocks.len(),
		}
	}

	/// Process new blocks inventory
	fn process_new_blocks_headers(&mut self, peer_index: usize, mut hashes: Vec<H256>, mut headers: Vec<BlockHeader>) {
		assert_eq!(hashes.len(), headers.len());

		let mut chain = self.chain.write();
		match chain.intersect_with_headers(&hashes, &headers) {
			HeadersIntersection::NoKnownBlocks(_) if self.state.is_synchronizing() => {
				warn!(target: "sync", "Ignoring {} headers from peer#{}. Unknown and we are synchronizing.", headers.len(), peer_index);
			},
			HeadersIntersection::DbAllBlocksKnown => {
				trace!(target: "sync", "Ignoring {} headers from peer#{}. All blocks are known and in database.", headers.len(), peer_index);
				if self.state.is_synchronizing() {
					// remember peer as useful
					self.peers.insert(peer_index);
				}
			},
			HeadersIntersection::InMemoryNoNewBlocks => {
				trace!(target: "sync", "Ignoring {} headers from peer#{}. All blocks are known and in memory.", headers.len(), peer_index);
				// remember peer as useful
				self.peers.insert(peer_index);
			},
			HeadersIntersection::InMemoryMainNewBlocks(new_block_index)
				| HeadersIntersection::InMemoryForkNewBlocks(new_block_index)
				| HeadersIntersection::DbForkNewBlocks(new_block_index)
				| HeadersIntersection::NoKnownBlocks(new_block_index) => {
				// schedule new blocks
				let new_blocks_hashes = hashes.split_off(new_block_index);
				let new_blocks_headers = headers.split_off(new_block_index);
				let new_blocks_hashes_len = new_blocks_hashes.len();
				chain.schedule_blocks_headers(new_blocks_hashes, new_blocks_headers);
				// remember peer as useful
				self.peers.insert(peer_index);
				// switch to synchronization state
				if !self.state.is_synchronizing() {
					if new_blocks_hashes_len == 1 && !self.state.is_nearly_saturated() {
						self.state = State::NearlySaturated;
					}
					else {
						self.state = State::Synchronizing(time::precise_time_s(), chain.best_storage_block().number);
					}
				}
			}
		}
	}

	/// Process new peer block
	fn process_peer_block(&mut self, peer_index: usize, block_hash: H256, block: Block) {
		let switch_to_saturated = {
			let mut chain = self.chain.write();
			match chain.block_state(&block_hash) {
				BlockState::Verifying | BlockState::Stored => {
					// remember peer as useful
					self.peers.insert(peer_index);
				},
				BlockState::Unknown | BlockState::Scheduled | BlockState::Requested => {
					// check parent block state
					match chain.block_state(&block.block_header.previous_header_hash) {
						BlockState::Unknown => {
							if self.state.is_synchronizing() {
								// when synchronizing, we tend to receive all blocks in-order
								trace!(target: "sync", "Ignoring block {} from peer#{}, because its parent is unknown and we are synchronizing", block_hash, peer_index);
								// remove block from current queue
								chain.forget(&block_hash);
								// remove orphaned blocks
								SynchronizationClient::<T>::remove_orphaned_blocks_for_parent(&mut self.unknown_blocks, &mut self.orphaned_blocks, &mut chain, &block_hash);
							} else {
								// remove this block from the queue
								chain.forget_leave_header(&block_hash);
								// remember this block as unknown
								self.unknown_blocks.insert(block_hash.clone(), time::precise_time_s());
								self.orphaned_blocks
									.entry(block.block_header.previous_header_hash.clone())
									.or_insert_with(HashMap::new)
									.insert(block_hash, block);
							}
						},
						BlockState::Verifying | BlockState::Stored => {
							// remember peer as useful
							self.peers.insert(peer_index);
							// forget block
							chain.forget_leave_header(&block_hash);
							// schedule verification
							let mut blocks: VecDeque<(H256, Block)> = VecDeque::new();
							blocks.push_back((block_hash.clone(), block));
							blocks.extend(SynchronizationClient::<T>::remove_orphaned_blocks_for_parent(&mut self.unknown_blocks, &mut self.orphaned_blocks, &mut chain, &block_hash));
							while let Some((block_hash, block)) = blocks.pop_front() {
								match self.verification_work_sender {
									Some(ref verification_work_sender) => {
										// remember that we are verifying block from this peer
										self.verifying_blocks_by_peer.insert(block_hash.clone(), peer_index);
										match self.verifying_blocks_waiters.entry(peer_index) {
											Entry::Occupied(mut entry) => {
												entry.get_mut().0.insert(block_hash.clone());
											},
											Entry::Vacant(entry) => {
												let mut block_hashes = HashSet::new();
												block_hashes.insert(block_hash.clone());
												entry.insert((block_hashes, None));
											}
										}
										// append to verifying queue
										chain.verify_block(block_hash.clone(), block.block_header.clone());
										// schedule verification
										verification_work_sender
											.send(VerificationTask::VerifyBlock(block))
											.expect("Verification thread have the same lifetime as `Synchronization`");
									},
									None => {
										// insert to the storage + forget block header
										chain.insert_best_block(block_hash.clone(), block)
											.expect("Error inserting to db.");
									},
								}
							}
						},
						BlockState::Requested | BlockState::Scheduled => {
							// remember peer as useful
							self.peers.insert(peer_index);
							// remember as orphan block
							self.orphaned_blocks
								.entry(block.block_header.previous_header_hash.clone())
								.or_insert_with(HashMap::new)
								.insert(block_hash, block);
						}
					}
				},
			}

			// requested block is received => move to saturated state if there are no more blocks
			chain.length_of_state(BlockState::Scheduled) == 0
				&& chain.length_of_state(BlockState::Requested) == 0
		};

		if switch_to_saturated {
			self.switch_to_saturated_state(true);
		}
	}

	/// Schedule new synchronization tasks, if any.
	fn execute_synchronization_tasks(&mut self, forced_blocks_requests: Option<Vec<H256>>) {
		let mut tasks: Vec<Task> = Vec::new();
		let idle_peers = self.peers.idle_peers();
		let idle_peers_len = idle_peers.len() as u32;

		// if some blocks requests are forced => we should ask peers even if there are no idle peers
		if let Some(forced_blocks_requests) = forced_blocks_requests {
			let all_peers = self.peers.all_peers();
			tasks.extend(self.prepare_blocks_requests_tasks(all_peers, forced_blocks_requests));
		}

		// prepare 'normal' synchronization tasks
		if idle_peers_len != 0 {
			let blocks_to_request = {
				// display information if processed many blocks || enough time has passed since sync start
				let mut chain = self.chain.write();
				if let State::Synchronizing(timestamp, num_of_blocks) = self.state {
					let new_timestamp = time::precise_time_s();
					let timestamp_diff = new_timestamp - timestamp;
					let new_num_of_blocks = chain.best_storage_block().number;
					let blocks_diff = if new_num_of_blocks > num_of_blocks { new_num_of_blocks - num_of_blocks } else { 0 };
					if timestamp_diff >= 60.0 || blocks_diff > 1000 {
						self.state = State::Synchronizing(time::precise_time_s(), new_num_of_blocks);

						use time;
						info!(target: "sync", "{:?} @ Processed {} blocks in {} seconds. Chain information: {:?}"
							, time::strftime("%H:%M:%S", &time::now()).unwrap()
							, blocks_diff, timestamp_diff
							, chain.information());
					}
				}

				// check if we can query some blocks hashes
				let scheduled_hashes_len = chain.length_of_state(BlockState::Scheduled);
				if self.state.is_synchronizing() {
					if scheduled_hashes_len < MAX_SCHEDULED_HASHES {
						tasks.push(Task::RequestBlocksHeaders(idle_peers[0]));
						self.peers.on_inventory_requested(idle_peers[0]);
					}
				}

				// check if we can move some blocks from scheduled to requested queue
				let requested_hashes_len = chain.length_of_state(BlockState::Requested);
				let verifying_hashes_len = chain.length_of_state(BlockState::Verifying);
				if requested_hashes_len + verifying_hashes_len < MAX_REQUESTED_BLOCKS + MAX_VERIFYING_BLOCKS && scheduled_hashes_len != 0 {
					let chunk_size = min(MAX_BLOCKS_IN_REQUEST, max(scheduled_hashes_len / idle_peers_len, MIN_BLOCKS_IN_REQUEST));
					let hashes_to_request_len = chunk_size * idle_peers_len;
					Some(chain.request_blocks_hashes(hashes_to_request_len))
				} else {
					None
				}
			};

			if let Some(blocks_to_request) = blocks_to_request {
				tasks.extend(self.prepare_blocks_requests_tasks(idle_peers, blocks_to_request));
			}
		}

		// execute synchronization tasks
		for task in tasks {
			self.executor.lock().execute(task);
		}
	}

	fn prepare_blocks_requests_tasks(&mut self, peers: Vec<usize>, mut hashes: Vec<H256>) -> Vec<Task> {
		use std::mem::swap;

		let chunk_size = min(MAX_BLOCKS_IN_REQUEST, max(hashes.len() as u32, MIN_BLOCKS_IN_REQUEST));
		let last_peer_index = peers.len() - 1;
		let mut tasks: Vec<Task> = Vec::new();
		for (peer_index, peer) in peers.into_iter().enumerate() {
			// we have to request all blocks => we will request last peer for all remaining blocks
			let peer_chunk_size = if peer_index == last_peer_index { hashes.len() } else { min(hashes.len(), chunk_size as usize) };
			if peer_chunk_size == 0 {
				break;
			}

			let mut chunk_hashes = hashes.split_off(peer_chunk_size);
			swap(&mut chunk_hashes, &mut hashes);

			self.peers.on_blocks_requested(peer, &chunk_hashes);
			tasks.push(Task::RequestBlocks(peer, chunk_hashes));
		}

		assert_eq!(hashes.len(), 0);
		tasks
	}

	/// Switch to saturated state
	fn switch_to_saturated_state(&mut self, ask_for_inventory: bool) {
		if self.state.is_saturated() {
			return;
		}

		self.state = State::Saturated;
		self.peers.reset();

		// remove sync orphans, but leave unknown orphans until they'll be removed by management thread
		let orphans_to_remove: HashSet<_> = self.orphaned_blocks.values()
			.flat_map(|v| v.iter().map(|e| e.0.clone()))
			.filter(|h| !self.unknown_blocks.contains_key(h))
			.collect();
		self.remove_orphaned_blocks(orphans_to_remove);

		// leave currently verifying blocks
		{
			let mut chain = self.chain.write();
			chain.forget_all_with_state(BlockState::Requested);
			chain.forget_all_with_state(BlockState::Scheduled);

			use time;
			info!(target: "sync", "{:?} @ Switched to saturated state. Chain information: {:?}",
				time::strftime("%H:%M:%S", &time::now()).unwrap(),
				chain.information());
		}

		if ask_for_inventory {
			let mut executor = self.executor.lock();
			for idle_peer in self.peers.idle_peers() {
				self.peers.on_inventory_requested(idle_peer);
				executor.execute(Task::RequestBlocksHeaders(idle_peer));
			}
		}
	}

	/// Remove orphaned blocks for given parent
	fn remove_orphaned_blocks_for_parent(unknown_blocks: &mut LinkedHashMap<H256, f64>, orphaned_blocks: &mut HashMap<H256, HashMap<H256, Block>>, chain: &mut Chain, parent: &H256) -> VecDeque<(H256, Block)> {
		let mut queue: VecDeque<H256> = VecDeque::new();
		queue.push_back(parent.clone());

		let mut removed: VecDeque<(H256, Block)> = VecDeque::new();
		while let Some(parent_hash) = queue.pop_front() {
			chain.forget_leave_header(&parent_hash);

			if let Entry::Occupied(entry) = orphaned_blocks.entry(parent_hash) {
				let (_, orphaned) = entry.remove_entry();
				for orphaned_hash in orphaned.keys() {
					unknown_blocks.remove(&orphaned_hash);
				}
				queue.extend(orphaned.keys().cloned());
				removed.extend(orphaned.into_iter());
			}
		}
		removed
	}

	/// Remove given orphaned blocks
	fn remove_orphaned_blocks(&mut self, orphans_to_remove: HashSet<H256>) {
		let parent_orphan_keys: Vec<_> = self.orphaned_blocks.keys().cloned().collect();
		for parent_orphan_key in parent_orphan_keys.into_iter() {
			if let Entry::Occupied(mut orphan_entry) = self.orphaned_blocks.entry(parent_orphan_key.clone()) {
				if {
					let mut orphans = orphan_entry.get_mut();
					let orphans_keys: HashSet<H256> = orphans.keys().cloned().collect();
					for orphan_to_remove in orphans_keys.intersection(&orphans_to_remove) {
						orphans.remove(orphan_to_remove);
					}
					orphans.is_empty()
				} {
					orphan_entry.remove_entry();
				}
			}
		}

		let mut chain = self.chain.write();
		for orphan_to_remove in orphans_to_remove {
			chain.forget(&orphan_to_remove);
		}
	}

	/// Awake threads, waiting for this block
	fn awake_waiting_threads(&mut self, hash: &H256) {
		// find a peer, which has supplied us with this block
		if let Entry::Occupied(block_entry) = self.verifying_blocks_by_peer.entry(hash.clone()) {
			let peer_index = *block_entry.get();
			// find a # of blocks, which this thread has supplied
			if let Entry::Occupied(mut entry) = self.verifying_blocks_waiters.entry(peer_index) {
				if {
					let &mut (ref mut waiting, ref waiter) = entry.get_mut();
					waiting.remove(hash);
					// if this is the last block => awake waiting threads
					let is_last_block = waiting.is_empty();
					if is_last_block {
						if let Some(ref waiter) = *waiter {
							waiter.awake();
						}
					}
					is_last_block
				} {
					entry.remove_entry();
				}
			}
			block_entry.remove_entry();
		}
	}

	/// Thread procedure for handling verification tasks
	fn verification_worker_proc(sync: Arc<Mutex<Self>>, storage: Arc<db::Store>, work_receiver: Receiver<VerificationTask>) {
		let verifier = ChainVerifier::new(storage);
		while let Ok(task) = work_receiver.recv() {
			match task {
				VerificationTask::VerifyBlock(block) => {
					match verifier.verify(&block) {
						Ok(_chain) => {
							sync.lock().on_block_verification_success(block)
						},
						Err(err) => {
							sync.lock().on_block_verification_error(&err, &block.hash())
						}
					}
				},
				_ => break,
			}
		}
	}
}

impl PeersBlocksWaiter {
	pub fn wait(&self) {
		let mut locker = self.peer_blocks_lock.lock();
		if *locker {
			return;
		}

		self.peer_blocks_done.wait(&mut locker);
	}

	pub fn awake(&self) {
		let mut locker = self.peer_blocks_lock.lock();
		*locker = true;
		self.peer_blocks_done.notify_all();
	}
}

#[cfg(test)]
pub mod tests {
	use std::sync::Arc;
	use parking_lot::{Mutex, RwLock};
	use tokio_core::reactor::{Core, Handle};
	use chain::{Block, RepresentH256};
	use super::{Client, Config, SynchronizationClient};
	use synchronization_executor::Task;
	use synchronization_chain::{Chain, ChainRef};
	use synchronization_executor::tests::DummyTaskExecutor;
	use p2p::event_loop;
	use test_data;
	use db;
	use devtools::RandomTempPath;

	fn create_disk_storage() -> Arc<db::Store> {
		let path = RandomTempPath::create_dir();
		Arc::new(db::Storage::new(path.as_path()).unwrap())
	}

	fn create_sync(storage: Option<Arc<db::Store>>) -> (Core, Handle, Arc<Mutex<DummyTaskExecutor>>, ChainRef, Arc<Mutex<SynchronizationClient<DummyTaskExecutor>>>) {
		let event_loop = event_loop();
		let handle = event_loop.handle();
		let storage = match storage {
			Some(storage) => storage,
			None => Arc::new(db::TestStorage::with_genesis_block()),
		};
		let chain = ChainRef::new(RwLock::new(Chain::new(storage.clone())));
		let executor = DummyTaskExecutor::new();
		let config = Config { threads_num: 1, skip_verification: true };

		let client = SynchronizationClient::new(config, &handle, executor.clone(), chain.clone());
		(event_loop, handle, executor, chain, client)
	} 

	#[test]
	fn synchronization_saturated_on_start() {
		let (_, _, _, _, sync) = create_sync(None);
		let sync = sync.lock();
		let info = sync.information();
		assert!(!info.state.is_synchronizing());
		assert_eq!(info.orphaned, 0);
	}

	#[test]
	fn synchronization_in_order_block_path_nearly_saturated() {
		let (_, _, executor, _, sync) = create_sync(None);

		let mut sync = sync.lock();
		let block1: Block = test_data::block_h1();
		let block2: Block = test_data::block_h2();

		sync.on_new_blocks_headers(5, vec![block1.block_header.clone()]);
		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks, vec![Task::RequestBlocks(5, vec![block1.hash()])]);
		assert!(sync.information().state.is_nearly_saturated());
		assert_eq!(sync.information().orphaned, 0);
		assert_eq!(sync.information().chain.scheduled, 0);
		assert_eq!(sync.information().chain.requested, 1);
		assert_eq!(sync.information().chain.stored, 1);
		assert_eq!(sync.information().peers.idle, 0);
		assert_eq!(sync.information().peers.active, 1);

		// push unknown block => will be queued as orphan
		sync.on_peer_block(5, block2);
		assert!(sync.information().state.is_nearly_saturated());
		assert_eq!(sync.information().orphaned, 1);
		assert_eq!(sync.information().chain.scheduled, 0);
		assert_eq!(sync.information().chain.requested, 1);
		assert_eq!(sync.information().chain.stored, 1);
		assert_eq!(sync.information().peers.idle, 0);
		assert_eq!(sync.information().peers.active, 1);

		// push requested block => should be moved to the test storage && orphan should be moved
		sync.on_peer_block(5, block1);
		assert!(sync.information().state.is_saturated());
		assert_eq!(sync.information().orphaned, 0);
		assert_eq!(sync.information().chain.scheduled, 0);
		assert_eq!(sync.information().chain.requested, 0);
		assert_eq!(sync.information().chain.stored, 3);
		// we have just requested new `inventory` from the peer => peer is forgotten
		assert_eq!(sync.information().peers.idle, 0);
		assert_eq!(sync.information().peers.active, 0);
	}

	#[test]
	fn synchronization_out_of_order_block_path() {
		let (_, _, _, _, sync) = create_sync(None);
		let mut sync = sync.lock();

		sync.on_new_blocks_headers(5, vec![test_data::block_h1().block_header.clone(), test_data::block_h2().block_header.clone()]);
		sync.on_peer_block(5, test_data::block_h169());

		// out-of-order block was presented by the peer
		assert!(sync.information().state.is_synchronizing());
		assert_eq!(sync.information().orphaned, 0);
		assert_eq!(sync.information().chain.scheduled, 0);
		assert_eq!(sync.information().chain.requested, 2);
		assert_eq!(sync.information().chain.stored, 1);
		// we have just requested new `inventory` from the peer => peer is forgotten
		assert_eq!(sync.information().peers.idle, 0);
		assert_eq!(sync.information().peers.active, 1);
		// TODO: check that peer is penalized
	}

	#[test]
	fn synchronization_parallel_peers() {
		let (_, _, executor, _, sync) = create_sync(None);

		let block1: Block = test_data::block_h1();
		let block2: Block = test_data::block_h2();

		{
			let mut sync = sync.lock();
			// not synchronizing after start
			assert!(sync.information().state.is_saturated());
			// receive inventory from new peer#1
			sync.on_new_blocks_headers(1, vec![block1.block_header.clone()]);
			assert_eq!(sync.information().chain.requested, 1);
			// synchronization has started && new blocks have been requested
			let tasks = executor.lock().take_tasks();
			assert!(sync.information().state.is_nearly_saturated());
			assert_eq!(tasks, vec![Task::RequestBlocks(1, vec![block1.hash()])]);
		}

		{
			let mut sync = sync.lock();
			// receive inventory from new peer#2
			sync.on_new_blocks_headers(2, vec![block1.block_header.clone(), block2.block_header.clone()]);
			assert_eq!(sync.information().chain.requested, 2);
			// synchronization has started && new blocks have been requested
			let tasks = executor.lock().take_tasks();
			assert!(sync.information().state.is_synchronizing());
			assert_eq!(tasks, vec![Task::RequestBlocksHeaders(2), Task::RequestBlocks(2, vec![block2.hash()])]);
		}

		{
			let mut sync = sync.lock();
			// receive block from peer#2
			sync.on_peer_block(2, block2);
			assert!(sync.information().chain.requested == 2
				&& sync.information().orphaned == 1);
			// receive block from peer#1
			sync.on_peer_block(1, block1);

			assert!(sync.information().chain.requested == 0
				&& sync.information().orphaned == 0
				&& sync.information().chain.stored == 3);
		}
	}

	#[test]
	fn synchronization_reset_when_peer_is_disconnected() {
		let (_, _, _, _, sync) = create_sync(None);

		// request new blocks
		{
			let mut sync = sync.lock();
			sync.on_new_blocks_headers(1, vec![test_data::block_h1().block_header]);
			assert!(sync.information().state.is_nearly_saturated());
		}

		// lost connection to peer => synchronization state lost
		{
			let mut sync = sync.lock();
			sync.on_peer_disconnected(1);
			assert!(sync.information().state.is_saturated());
		}
	}

	#[test]
	fn synchronization_not_starting_when_receiving_known_blocks() {
		let (_, _, executor, _, sync) = create_sync(None);
		let mut sync = sync.lock();
		// saturated => receive inventory with known blocks only
		sync.on_new_blocks_headers(1, vec![test_data::genesis().block_header]);
		// => no need to start synchronization
		assert!(!sync.information().state.is_nearly_saturated());
		// => no synchronization tasks are scheduled
		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks, vec![]);
	}

	#[test]
	fn synchronization_asks_for_inventory_after_saturating() {
		let (_, _, executor, _, sync) = create_sync(None);
		let mut sync = sync.lock();
		let block = test_data::block_h1();
		sync.on_new_blocks_headers(1, vec![block.block_header.clone()]);
		sync.on_new_blocks_headers(2, vec![block.block_header.clone()]);
		executor.lock().take_tasks();
		sync.on_peer_block(2, block);

		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks.len(), 2);
		assert!(tasks.iter().any(|t| t == &Task::RequestBlocksHeaders(1)));
		assert!(tasks.iter().any(|t| t == &Task::RequestBlocksHeaders(2)));
	}

	#[test]
	fn synchronization_remembers_correct_block_headers_in_order() {
		let (_, _, executor, chain, sync) = create_sync(None);
		let mut sync = sync.lock();

		let b1 = test_data::block_h1();
		let b2 = test_data::block_h2();
		sync.on_new_blocks_headers(1, vec![b1.block_header.clone(), b2.block_header.clone()]);

		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks.len(), 2);
		assert!(tasks.iter().any(|t| t == &Task::RequestBlocksHeaders(1)));
		assert!(tasks.iter().any(|t| t == &Task::RequestBlocks(1, vec![b1.hash(), b2.hash()])));

		{
			let chain = chain.read();
			assert_eq!(chain.information().headers.best, 2);
			assert_eq!(chain.information().headers.total, 2);
		}

		sync.on_peer_block(1, b1);

		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks, vec![]);

		{
			let chain = chain.read();
			assert_eq!(chain.information().headers.best, 1);
			assert_eq!(chain.information().headers.total, 1);
		}

		sync.on_peer_block(1, b2);

		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks, vec![Task::RequestBlocksHeaders(1)]);

		{
			let chain = chain.read();
			assert_eq!(chain.information().headers.best, 0);
			assert_eq!(chain.information().headers.total, 0);
		}
	}

	#[test]
	fn synchronization_remembers_correct_block_headers_out_of_order() {
		let (_, _, executor, chain, sync) = create_sync(None);
		let mut sync = sync.lock();

		let b1 = test_data::block_h1();
		let b2 = test_data::block_h2();
		sync.on_new_blocks_headers(1, vec![b1.block_header.clone(), b2.block_header.clone()]);

		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks.len(), 2);
		assert!(tasks.iter().any(|t| t == &Task::RequestBlocksHeaders(1)));
		assert!(tasks.iter().any(|t| t == &Task::RequestBlocks(1, vec![b1.hash(), b2.hash()])));

		{
			let chain = chain.read();
			assert_eq!(chain.information().headers.best, 2);
			assert_eq!(chain.information().headers.total, 2);
		}

		sync.on_peer_block(1, b2);

		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks, vec![]);

		{
			let chain = chain.read();
			assert_eq!(chain.information().headers.best, 2);
			assert_eq!(chain.information().headers.total, 2);
		}

		sync.on_peer_block(1, b1);

		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks, vec![Task::RequestBlocksHeaders(1)]);

		{
			let chain = chain.read();
			assert_eq!(chain.information().headers.best, 0);
			assert_eq!(chain.information().headers.total, 0);
		}
	}

	#[test]
	fn synchronization_ignores_unknown_block_headers() {
		let (_, _, executor, chain, sync) = create_sync(None);
		let mut sync = sync.lock();

		let b169 = test_data::block_h169();
		sync.on_new_blocks_headers(1, vec![b169.block_header]);

		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks, vec![]);

		let chain = chain.read();
		assert_eq!(chain.information().headers.best, 0);
		assert_eq!(chain.information().headers.total, 0);
	}

	#[test]
	fn synchronization_works_for_forks_from_db_best_block() {
		let storage = create_disk_storage();
		let genesis = test_data::genesis();
		storage.insert_block(&genesis).expect("no db error");

		let (_, _, executor, chain, sync) = create_sync(Some(storage));
		let genesis_header = &genesis.block_header;
		let fork1 = test_data::build_n_empty_blocks_from(2, 100, &genesis_header);
		let fork2 = test_data::build_n_empty_blocks_from(3, 200, &genesis_header);

		let mut sync = sync.lock();
		sync.on_new_blocks_headers(1, vec![fork1[0].block_header.clone(), fork1[1].block_header.clone()]);
		sync.on_new_blocks_headers(2, vec![fork2[0].block_header.clone(), fork2[1].block_header.clone(), fork2[2].block_header.clone()]);

		let tasks = { executor.lock().take_tasks() };
		assert_eq!(tasks, vec![Task::RequestBlocksHeaders(1),
			Task::RequestBlocks(1, vec![fork1[0].hash(), fork1[1].hash()]),
			Task::RequestBlocksHeaders(2),
			Task::RequestBlocks(2, vec![fork2[0].hash(), fork2[1].hash(), fork2[2].hash()]),
		]);

		sync.on_peer_block(2, fork2[0].clone());
		{
			let chain = chain.read();
			assert_eq!(chain.best_storage_block().hash, fork2[0].hash());
			assert_eq!(chain.best_storage_block().number, 1);
		}

		sync.on_peer_block(1, fork1[0].clone());
		{
			let chain = chain.read();
			assert_eq!(chain.best_storage_block().hash, fork2[0].hash());
			assert_eq!(chain.best_storage_block().number, 1);
		}

		sync.on_peer_block(1, fork1[1].clone());
		{
			let chain = chain.read();
			assert_eq!(chain.best_storage_block().hash, fork1[1].hash());
			assert_eq!(chain.best_storage_block().number, 2);
		}

		sync.on_peer_block(2, fork2[1].clone());
		{
			let chain = chain.read();
			assert_eq!(chain.best_storage_block().hash, fork1[1].hash());
			assert_eq!(chain.best_storage_block().number, 2);
		}

		sync.on_peer_block(2, fork2[2].clone());
		{
			let chain = chain.read();
			assert_eq!(chain.best_storage_block().hash, fork2[2].hash());
			assert_eq!(chain.best_storage_block().number, 3);
		}
	}

	#[test]
	fn synchronization_works_for_forks_long_after_short() {
		let storage = create_disk_storage();
		let genesis = test_data::genesis();
		storage.insert_block(&genesis).expect("no db error");

		let (_, _, executor, chain, sync) = create_sync(Some(storage));
		let common_block = test_data::block_builder().header().parent(genesis.hash()).build().build();
		let fork1 = test_data::build_n_empty_blocks_from(2, 100, &common_block.block_header);
		let fork2 = test_data::build_n_empty_blocks_from(3, 200, &common_block.block_header);

		let mut sync = sync.lock();
		sync.on_new_blocks_headers(1, vec![common_block.block_header.clone(), fork1[0].block_header.clone(), fork1[1].block_header.clone()]);
		sync.on_new_blocks_headers(2, vec![common_block.block_header.clone(), fork2[0].block_header.clone(), fork2[1].block_header.clone(), fork2[2].block_header.clone()]);

		let tasks = { executor.lock().take_tasks() };
		assert_eq!(tasks, vec![Task::RequestBlocksHeaders(1),
			Task::RequestBlocks(1, vec![common_block.hash(), fork1[0].hash(), fork1[1].hash()]),
			Task::RequestBlocksHeaders(2),
			Task::RequestBlocks(2, vec![fork2[0].hash(), fork2[1].hash(), fork2[2].hash()]),
		]);

		// TODO: this will change from 3 to 4 after longest fork will be stored in the BestHeadersChain
		// however id doesn't affect sync process, as it is shown below
		{
			let chain = chain.read();
			assert_eq!(chain.information().headers.best, 3);
			assert_eq!(chain.information().headers.total, 3);
		}

		sync.on_peer_block(1, common_block.clone());
		sync.on_peer_block(1, fork1[0].clone());
		sync.on_peer_block(1, fork1[1].clone());
		sync.on_peer_block(2, fork2[0].clone());
		sync.on_peer_block(2, fork2[1].clone());
		sync.on_peer_block(2, fork2[2].clone());

		{
			let chain = chain.read();
			assert_eq!(chain.best_storage_block().hash, fork2[2].hash());
			assert_eq!(chain.best_storage_block().number, 4);
		}
	}

	#[test]
	fn accept_out_of_order_blocks_when_saturated() {
		let (_, _, _, chain, sync) = create_sync(None);
		let mut sync = sync.lock();

		sync.on_peer_block(1, test_data::block_h2());
		assert_eq!(sync.information().orphaned, 1);

		{
			let chain = chain.read();
			assert_eq!(chain.best_storage_block().number, 0);
		}

		sync.on_peer_block(1, test_data::block_h1());
		assert_eq!(sync.information().orphaned, 0);

		{
			let chain = chain.read();
			assert_eq!(chain.best_storage_block().number, 2);
		}
	}

	#[test]
	fn do_not_rerequest_unknown_block_in_inventory() {
		let (_, _, executor, _, sync) = create_sync(None);
		let mut sync = sync.lock();

		sync.on_peer_block(1, test_data::block_h2());
		sync.on_new_blocks_inventory(1, vec![test_data::block_h1().hash(), test_data::block_h2().hash()]);

		let tasks = executor.lock().take_tasks();
		assert_eq!(tasks, vec![Task::RequestBlocks(1, vec![test_data::block_h1().hash()])]);
	}
}