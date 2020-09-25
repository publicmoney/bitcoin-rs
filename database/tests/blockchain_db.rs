extern crate chain;
extern crate db;
extern crate storage;
extern crate test_data;

use bitcrypto::SHA256D;
use chain::IndexedBlock;
use db::blockchain_db::BlockChainDatabase;
use db::ham_adapter::HamDb;
use storage::{BlockProvider, ForkChain, SideChainOrigin, Store};

#[test]
fn insert_block() {
	let store = BlockChainDatabase::transient().unwrap();
	let b0: IndexedBlock = test_data::block_h0().into();
	let b1: IndexedBlock = test_data::block_h1().into();
	let b2: IndexedBlock = test_data::block_h2().into();

	store.insert(b0.clone()).unwrap();
	store.insert(b1.clone()).unwrap();
	store.insert(b2.clone()).unwrap();

	assert_eq!(0, store.best_block().number);
	assert_eq!(SHA256D::default(), store.best_block().hash);

	store.canonize(b0.hash()).unwrap();
	assert_eq!(0, store.best_block().number);
	assert_eq!(b0.hash(), &store.best_block().hash);

	store.canonize(b1.hash()).unwrap();
	assert_eq!(1, store.best_block().number);
	assert_eq!(b1.hash(), &store.best_block().hash);

	store.canonize(b2.hash()).unwrap();
	assert_eq!(2, store.best_block().number);
	assert_eq!(b2.hash(), &store.best_block().hash);

	let decanonized = store.decanonize().unwrap();
	assert_eq!(b2.hash(), &decanonized);
	assert_eq!(1, store.best_block().number);
	assert_eq!(b1.hash(), &store.best_block().hash);

	assert_eq!(b0.hash(), &store.block_hash(0).unwrap());
	assert_eq!(b1.hash(), &store.block_hash(1).unwrap());

	assert_eq!(0, store.block_number(b0.hash()).unwrap());
	assert_eq!(1, store.block_number(b1.hash()).unwrap());
}

#[test]
fn reopen_db() {
	std::fs::remove_dir_all("testdb").unwrap_or_default();

	let b0: IndexedBlock = test_data::block_h0().into();
	let b1: IndexedBlock = test_data::block_h1().into();
	let b2: IndexedBlock = test_data::block_h2().into();
	{
		let ham = HamDb::persistent("testdb", "reopen", 100).unwrap();
		let store = BlockChainDatabase::open(ham.clone()).unwrap();
		store.insert(b0.clone()).unwrap();
		store.insert(b1.clone()).unwrap();
		store.insert(b2.clone()).unwrap();

		store.canonize(b0.hash()).unwrap();
		store.canonize(b1.hash()).unwrap();
		store.canonize(b2.hash()).unwrap();

		store.decanonize().unwrap();
		store.shutdown();

		assert_eq!(1, store.best_block().number);
	}
	{
		let ham = HamDb::persistent("testdb", "reopen", 100).unwrap();
		let store = BlockChainDatabase::open(ham).unwrap();
		assert_eq!(b0.hash(), &store.block_hash(0).unwrap());
		assert_eq!(1, store.best_block().number);
		assert_eq!(b1.hash(), &store.best_block().hash);
	}
}

#[test]
fn switch_to_simple_fork() {
	let store = BlockChainDatabase::transient().unwrap();
	let b0: IndexedBlock = test_data::block_h0().into();
	let b1: IndexedBlock = test_data::block_h1().into();
	let b2: IndexedBlock = test_data::block_h2().into();

	store.insert(b0.clone()).unwrap();
	store.insert(b1.clone()).unwrap();
	store.insert(b2.clone()).unwrap();

	store.canonize(b0.hash()).unwrap();
	store.canonize(b1.hash()).unwrap();

	assert_eq!(1, store.best_block().number);
	assert_eq!(b1.hash(), &store.best_block().hash);

	let side_chain_origin = SideChainOrigin {
		ancestor: 1,
		canonized_route: Vec::new(),
		decanonized_route: Vec::new(),
		block_number: 2,
	};

	let fork = store.fork(side_chain_origin).unwrap();
	assert_eq!(1, fork.store().best_block().number);
	assert_eq!(b1.hash(), &fork.store().best_block().hash);

	fork.store().canonize(b2.hash()).unwrap();
	store.switch_to_fork(fork).unwrap();

	assert_eq!(2, store.best_block().number);
	assert_eq!(b2.hash(), &store.best_block().hash);

	let side_chain_origin = SideChainOrigin {
		ancestor: 1,
		canonized_route: Vec::new(),
		decanonized_route: vec![b2.hash().clone()],
		block_number: 2,
	};

	let fork = store.fork(side_chain_origin).unwrap();
	let decanonized = fork.store().decanonize().unwrap();
	assert_eq!(b1.hash(), &decanonized);

	assert_eq!(0, fork.store().best_block().number);
	assert_eq!(b0.hash(), &fork.store().best_block().hash);

	assert_eq!(2, store.best_block().number);
	assert_eq!(b2.hash(), &store.best_block().hash);
	assert_eq!(store.best_block().hash, store.block_hash(2).unwrap());
}
