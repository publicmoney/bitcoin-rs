extern crate chain;
extern crate criterion;
extern crate storage;
extern crate test_data;

use bitcrypto::SHA256D;
use chain::IndexedBlock;
use criterion::{criterion_group, criterion_main, Criterion};

use database::adapator::BlockChainDatabase;
use database::persistent;

// which is better performance - keyed lookup or two lookups by ref? Thinking about getting blocks txs from list
// 1. write 12000 blocks
// 2. write 100 blocks that has 100 transaction each spending outputs from first 1000 blocks
pub fn write_heavy(c: &mut Criterion) {
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
			// let db = transient(10).unwrap();
			let db = persistent("bench", 8, 128).unwrap();
			let mut store = BlockChainDatabase::open(db).unwrap();

			store.insert(&genesis).unwrap();
			store.canonize(&genesis.header.hash).unwrap();
			for block in &blocks {
				let block: IndexedBlock = block.clone().into();
				let hash = block.hash().clone();
				store.insert(&block).expect("cannot insert bench block");
				store.canonize(&hash).unwrap();
			}
		})
	});
}

pub fn key_speed(c: &mut Criterion) {
	c.bench_function("key speed", |b| {
		let mut db = persistent("bench", 0, 128).unwrap();

		for x in 0..1000 {
			let key = format!("abc{}", x);
			db.put_keyed(key.as_ref(), "hello".as_ref()).unwrap();
		}
		b.iter(|| {
			db.get_keyed("abc500".as_ref()).unwrap();
		})
	});

	c.bench_function("get speed", |b| {
		let mut db = persistent("bench", 0, 128).unwrap();
		for x in 0..1000 {
			db.put("hello".as_ref()).unwrap();
		}
		let pref = db.put("hello".as_ref()).unwrap();
		b.iter(|| {
			db.get(pref).unwrap();
		})
	});
}
criterion_group! {
	name = benches;
	// This can be any expression that returns a `Criterion` object.
	config = Criterion::default().significance_level(0.1).sample_size(10);
	targets = key_speed
}
criterion_main!(benches);
