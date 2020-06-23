extern crate chain;
extern crate criterion;
extern crate db;
extern crate storage;
extern crate test_data;

use chain::IndexedBlock;
use criterion::{criterion_group, criterion_main, Criterion};
use db::BlockChainDatabase;
use storage::{BlockOrigin, BlockProvider, BlockRef, ForkChain};

pub fn fetch(c: &mut Criterion) {
	// params
	const BLOCKS: usize = 1000;

	// test setup
	let genesis: IndexedBlock = test_data::genesis().into();
	let store = BlockChainDatabase::init_test_chain(vec![genesis.clone()]);
	let mut rolling_hash: chain::hash::H256 = *genesis.hash();

	let mut blocks = Vec::new();
	let mut hashes = Vec::new();

	for i in 0..BLOCKS {
		#[rustfmt::skip]
		let next_block = test_data::block_builder()
			.transaction()
				.coinbase()
				.lock_time(i as u32)
				.output().value(5000000000).build()
				.build()
			.merkled_header().parent(rolling_hash).nonce(i as u32).build()
			.build();
		rolling_hash = next_block.hash();
		blocks.push(next_block);
		hashes.push(rolling_hash.clone());
	}

	for block in blocks.into_iter() {
		let block: IndexedBlock = block.into();
		store.insert(block.clone()).unwrap();
		store.canonize(block.hash()).unwrap();
	}

	c.bench_function("fetch", |b| {
		b.iter(|| {
			for i in 0..BLOCKS {
				let block = store.block(BlockRef::Hash(hashes[i])).unwrap();
				assert_eq!(block.hash(), &hashes[i]);
			}
		})
	});
}

pub fn write(c: &mut Criterion) {
	// params
	const BLOCKS: usize = 1000;

	// setup
	let genesis: IndexedBlock = test_data::genesis().into();
	let mut rolling_hash: chain::hash::H256 = *genesis.hash();

	let mut blocks: Vec<IndexedBlock> = Vec::new();

	for i in 0..BLOCKS {
		#[rustfmt::skip]
		let next_block = test_data::block_builder()
			.transaction()
				.coinbase()
				.lock_time(i as u32)
					.output()
					.value(5000000000)
					.build()
				.build()
			.merkled_header()
				.parent(rolling_hash)
				.nonce(i as u32)
				.build()
			.build();
		rolling_hash = next_block.hash();
		blocks.push(next_block.into());
	}

	// bench
	c.bench_function("write", |b| {
		b.iter(|| {
			let store = BlockChainDatabase::init_test_chain(vec![genesis.clone()]);
			for block in &blocks {
				let hash = block.hash().clone();
				store.insert(block.clone()).unwrap();
				store.canonize(&hash).unwrap();
			}
		})
	});
}

pub fn reorg_short(c: &mut Criterion) {
	// params
	const BLOCKS: usize = 1000;

	// setup
	let genesis: IndexedBlock = test_data::genesis().into();
	let mut rolling_hash: chain::hash::H256 = *genesis.hash();

	let mut blocks = Vec::new();

	for x in 0..BLOCKS {
		let base = rolling_hash.clone();
		#[rustfmt::skip]
		let next_block = test_data::block_builder()
			.transaction()
				.coinbase()
				.lock_time(x as u32)
				.output().value(5000000000).build()
				.build()
			.merkled_header().parent(rolling_hash).nonce(x as u32 * 4).build()
			.build();
		rolling_hash = next_block.hash();
		blocks.push(next_block);
		#[rustfmt::skip]
		let next_block_side = test_data::block_builder()
			.transaction()
				.coinbase()
				.lock_time(x as u32)
				.output().value(5000000000).build()
				.build()
			.merkled_header().parent(base).nonce(x as u32 * 4 + 2).build()
			.build();
		let next_base = next_block_side.hash();
		blocks.push(next_block_side);
		#[rustfmt::skip]
		let next_block_side_continue = test_data::block_builder()
			.transaction()
				.coinbase()
				.lock_time(x as u32)
				.output().value(5000000000).build()
				.build()
			.merkled_header().parent(next_base).nonce(x as u32 * 4 + 3).build()
			.build();
		blocks.push(next_block_side_continue);
		#[rustfmt::skip]
		let next_block_continue = test_data::block_builder()
			.transaction()
				.coinbase()
				.lock_time(x as u32)
				.output().value(5000000000).build()
				.build()
			.merkled_header().parent(rolling_hash).nonce(x as u32 * 4 + 1).build()
			.build();
		rolling_hash = next_block_continue.hash();
		blocks.push(next_block_continue);
	}

	let mut total: usize = 0;
	let mut reorgs: usize = 0;

	// bench
	c.bench_function("reorg_short", |b| {
		b.iter(|| {
			let store = BlockChainDatabase::init_test_chain(vec![genesis.clone()]);
			for idx in 0..BLOCKS {
				total += 1;
				let block: IndexedBlock = blocks[idx].clone().into();
				let hash = block.hash().clone();

				match store.block_origin(&block.header).unwrap() {
					BlockOrigin::KnownBlock => {
						unreachable!();
					}
					BlockOrigin::CanonChain { .. } => {
						store.insert(block).unwrap();
						store.canonize(&hash).unwrap();
					}
					BlockOrigin::SideChain(_origin) => {
						store.insert(block).unwrap();
					}
					BlockOrigin::SideChainBecomingCanonChain(origin) => {
						reorgs += 1;
						let fork = store.fork(origin).unwrap();
						fork.store().insert(block).unwrap();
						fork.store().canonize(&hash).unwrap();
						store.switch_to_fork(fork).unwrap();
					}
				}
			}
		})
	});

	// reorgs occur twice per iteration except last one where there only one, blocks are inserted with rate 4/iteration
	// so reorgs = total/2 - 1
	assert_eq!(total % 1000, 0);
	assert_eq!(reorgs % 499, 0);
}

// 1. write 12000 blocks
// 2. write 100 blocks that has 100 transaction each spending outputs from first 1000 blocks
pub fn write_heavy(c: &mut Criterion) {
	// params
	const BLOCKS_INITIAL: usize = 1200;
	const BLOCKS: usize = 10;
	const TRANSACTIONS: usize = 10;
	// test setup
	let genesis: IndexedBlock = test_data::genesis().into();

	let mut rolling_hash: chain::hash::H256 = *genesis.hash();
	let mut blocks = Vec::new();
	let mut hashes = Vec::new();

	for x in 0..BLOCKS_INITIAL {
		#[rustfmt::skip]
		let next_block = test_data::block_builder()
			.transaction()
				.coinbase()
				.lock_time(x as u32)
				.output()
					.value(5000000000)
					.build()
				.build()
			.merkled_header()
				.parent(rolling_hash)
				.nonce(x as u32)
				.build()
			.build();
		rolling_hash = next_block.hash();
		blocks.push(next_block);
		hashes.push(rolling_hash.clone());
	}

	for b in 0..BLOCKS {
		let mut builder = test_data::block_builder().transaction().coinbase().build();

		for t in 0..TRANSACTIONS {
			builder = builder
				.transaction()
				.input()
				.hash(blocks[b * TRANSACTIONS + t].transactions()[0].hash())
				.build() // default index is 0 which is ok
				.output()
				.value(1000)
				.build()
				.build();
		}

		let next_block = builder.merkled_header().parent(rolling_hash).build().build();

		rolling_hash = next_block.hash();
		blocks.push(next_block);
		hashes.push(rolling_hash.clone());
	}

	// bench
	c.bench_function("write_heavy", |b| {
		b.iter(|| {
			let store = BlockChainDatabase::init_test_chain(vec![genesis.clone()]);
			for block in &blocks {
				let block: IndexedBlock = block.clone().into();
				let hash = block.hash().clone();
				store.insert(block).expect("cannot insert bench block");
				store.canonize(&hash).unwrap();
			}
		})
	});
}

criterion_group!(benches, fetch, write, reorg_short, write_heavy);
criterion_main!(benches);
