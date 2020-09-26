extern crate chain;
extern crate criterion;
extern crate storage;
extern crate test_data;

use bitcrypto::SHA256D;
use chain::IndexedBlock;
use criterion::{criterion_group, criterion_main, Criterion};

use db::blockchain_db::BlockChainDatabase;

const TEST_DB: &'static str = "testdb/bench";

// which is better performance - keyed lookup or two lookups by ref? Thinking about getting blocks txs from list
// 1. write 12000 blocks
// 2. write 100 blocks that has 100 transaction each spending outputs from first 1000 blocks
pub fn write_heavy(c: &mut Criterion) {
	let _ = std::fs::remove_dir_all(TEST_DB);

	// params
	const BLOCKS_INITIAL: usize = 100;
	const BLOCKS: usize = 10;
	const TRANSACTIONS: usize = 10;
	// test setup
	let genesis: IndexedBlock = test_data::genesis().into();

	let mut rolling_hash: SHA256D = *genesis.hash();
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
			let _ = std::fs::remove_dir_all(TEST_DB.to_string());

			let store = BlockChainDatabase::persistent(&TEST_DB.to_string(), 100).unwrap();

			store.insert(genesis.clone()).unwrap();
			store.canonize(&genesis.header.hash).unwrap();
			for block in &blocks {
				let block: IndexedBlock = block.clone().into();
				let hash = block.hash().clone();
				store.insert(block).expect("cannot insert bench block");
				store.canonize(&hash).unwrap();
			}
		})
	});
}

criterion_group! {
	name = benches;
	// This can be any expression that returns a `Criterion` object.
	config = Criterion::default().significance_level(0.1).sample_size(10);
	targets = write_heavy
}
criterion_main!(benches);
