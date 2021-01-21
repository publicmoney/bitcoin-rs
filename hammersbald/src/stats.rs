use crate::format::Payload;
use crate::hammersbald_api::Hammersbald;

use bitcoin_hashes::siphash24;
use log::info;
use std::collections::{HashMap, HashSet};

/// print some statistics on a db
pub fn stats(db: &mut Hammersbald) {
	let (step, log_mod, blen, tlen, dlen, llen, sip0, sip1) = db.params();
	info!("File sizes: table: {}, data: {}, links: {}", tlen, dlen, llen);
	info!("Hash table: buckets: {}, log_mod: {}, step: {}", blen, log_mod, step);

	let mut pointers = HashSet::new();
	let mut hashes = HashSet::new();
	let mut roots = HashMap::new();
	let mut n_links = 0;
	let mut used_buckets = 0;
	for (pref, bucket) in db.buckets() {
		if pref.is_valid() {
			pointers.insert(pref);

			if bucket.len() > 0 {
				used_buckets += 1;
				n_links += bucket.len();
			}
			for slot in bucket.into_iter() {
				if slot.1.is_valid() {
					roots.entry(slot.1).or_insert(Vec::new()).push(slot.0);
					hashes.insert(slot.0);
				}
			}
		}
	}
	info!(
		"Used buckets: {}. {:.1}% in use. Links per bucket: {:.1}",
		used_buckets,
		100.0 * (used_buckets as f32 / blen as f32),
		n_links as f32 / used_buckets as f32
	);

	let mut n_links = 0;
	for (pos, envelope) in db.link_envelopes() {
		match envelope.payload().unwrap() {
			Payload::Link(_) => {
				n_links += 1;
				pointers.remove(&pos);
			}
			_ => panic!("Unexpected payload type link at {}", pos),
		}
	}
	if !pointers.is_empty() {
		panic!("{} roots point to non-existent links", pointers.len());
	}

	info!("Data: indexed: {}, hash collisions: {:.2}", n_links, n_links - hashes.len());

	let mut indexed_garbage = 0;
	let mut referred = 0;
	for (pos, envelope) in db.data_envelopes() {
		match envelope.payload() {
			Ok(Payload::Indexed(indexed)) => {
				if let Some(root) = roots.remove(&pos) {
					let h = hash(indexed.key, sip0, sip1);
					if root.iter().any(|hash| *hash == h) == false {
						panic!("ERROR root {} points data with different key hash", pos);
					}
				} else {
					indexed_garbage += 1;
				}
			}
			Ok(Payload::Referred(_data)) => {
				referred += 1;
			}
			Ok(Payload::Link(_)) => panic!("Unexpected payload type (link) in data at {}", pos),
			Err(e) => panic!("{}", e),
		}
	}
	info!("Referred: {}", referred);
	info!("Garbage indexed: {}, links: {}", indexed_garbage, n_links - used_buckets);
	if !roots.is_empty() {
		panic!("ERROR {} roots point to non-existent data", roots.len());
	}
}

fn hash(key: &[u8], sip0: u64, sip1: u64) -> u64 {
	siphash24::Hash::hash_to_u64_with_keys(sip0, sip1, key) as u64
}

#[test]
fn test_stats() {
	let mut db = super::transient().unwrap();
	db.put_keyed(&[5], &[5]).unwrap();
	db.batch().unwrap();
	db.stats();
}
