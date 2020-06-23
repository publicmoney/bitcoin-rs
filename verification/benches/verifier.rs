extern crate byteorder;
extern crate chain;
extern crate criterion;
extern crate db;
extern crate network;
extern crate test_data;
extern crate verification;

use byteorder::{ByteOrder, LittleEndian};
use chain::IndexedBlock;
use criterion::{criterion_group, criterion_main, Criterion};
use db::BlockChainDatabase;
use network::{ConsensusParams, Network};
use std::sync::Arc;
use verification::{BackwardsCompatibleChainVerifier as ChainVerifier, VerificationLevel, Verify};

// 1. write BLOCKS_INITIAL blocks with 1 transaction each
// 2. verify <BLOCKS> blocks that has <TRANSACTIONS> transaction each with <OUTPUTS> output each,
//    spending outputs from last <BLOCKS*TRANSACTIONS*OUTPUTS> blocks
pub fn verifier(c: &mut Criterion) {
	// params
	const BLOCKS_INITIAL: usize = 5200;
	const BLOCKS: usize = 20;
	const TRANSACTIONS: usize = 20;
	const OUTPUTS: usize = 10;

	assert!(
		BLOCKS_INITIAL - 100 > BLOCKS * OUTPUTS * TRANSACTIONS,
		"There will be not enough initial blocks to continue this bench"
	);

	// test setup
	let genesis = test_data::genesis();

	let mut rolling_hash = genesis.hash();
	let mut blocks: Vec<IndexedBlock> = Vec::new();

	for x in 0..BLOCKS_INITIAL {
		let mut coinbase_nonce = [0u8; 8];
		LittleEndian::write_u64(&mut coinbase_nonce[..], x as u64);
		#[rustfmt::skip]
		let next_block = test_data::block_builder()
			.transaction()
				.lock_time(x as u32)
				.input()
					.coinbase()
					.signature_bytes(coinbase_nonce.to_vec().into())
					.build()
				.output().value(5000000000).build()
				.build()
			.merkled_header()
				.parent(rolling_hash.clone())
				.nonce(x as u32)
				.build()
			.build();
		rolling_hash = next_block.hash();
		blocks.push(next_block.into());
	}

	let store = Arc::new(BlockChainDatabase::init_test_chain(vec![genesis.clone().into()]));
	for block in blocks.iter() {
		let hash = block.hash().clone();
		store.insert(block.clone()).unwrap();
		store.canonize(&hash).unwrap();
	}

	let mut verification_blocks: Vec<IndexedBlock> = Vec::new();
	for b in 0..BLOCKS {
		let mut coinbase_nonce = [0u8; 8];
		LittleEndian::write_u64(&mut coinbase_nonce[..], (b + BLOCKS_INITIAL) as u64);
		#[rustfmt::skip]
		let mut builder = test_data::block_builder()
			.transaction()
				.lock_time(b as u32)
				.input().coinbase().signature_bytes(coinbase_nonce.to_vec().into()).build()
				.output().value(5000000000).build()
				.build();

		for t in 0..TRANSACTIONS {
			let mut tx_builder = builder.transaction();

			for o in 0..OUTPUTS {
				let parent_hash = blocks[(b * TRANSACTIONS * OUTPUTS + t * OUTPUTS + o)].transactions[0].hash.clone();

				tx_builder = tx_builder.input().hash(parent_hash).index(0).build()
			}

			builder = tx_builder.output().value(0).build().build()
		}
		#[rustfmt::skip]
		verification_blocks.push(
			builder
				.merkled_header()
					.parent(rolling_hash.clone())
					.build()
				.build()
			.into());
	}

	assert_eq!(store.best_block().hash, rolling_hash);

	let chain_verifier = ChainVerifier::new(store.clone(), ConsensusParams::new(Network::Unitest));

	// bench
	c.bench_function("verifier", |b| {
		b.iter(|| {
			for block in verification_blocks.iter() {
				chain_verifier.verify(VerificationLevel::Full, block).unwrap();
			}
		})
	});
}

criterion_group!(benches, verifier);
criterion_main!(benches);
