//
// Copyright 2018-2019 Tamas Blummer
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
//!
//! # Statistics for a Hammersbald db
//!
//!
use crate::api::Hammersbald;
use crate::format::Payload;

use bitcoin_hashes::siphash24;
use log::info;
use std::collections::{HashMap, HashSet};

/// print some statistics on a db
pub fn stats(db: &Hammersbald) {
	let (step, log_mod, blen, tlen, dlen, llen, sip0, sip1) = db.params();
	info!("File sizes: table: {}, data: {}, links: {}", tlen, dlen, llen);
	info!("Hash table: buckets: {}, log_mod: {}, step: {}", blen, log_mod, step);

	let mut pointers = HashSet::new();
	for bucket in db.buckets() {
		if bucket.is_valid() {
			pointers.insert(bucket);
		}
	}

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

	let mut roots = HashMap::new();
	let mut n_slots = 0;
	let mut used_buckets = 0;
	for slots in db.slots() {
		n_slots += slots.len();
		if slots.len() > 0 {
			used_buckets += 1;
		}
		for slot in slots.iter() {
			roots.entry(slot.1).or_insert(Vec::new()).push(slot.0);
		}
	}
	info!(
		"Used buckets: {}. {:.1}% average filled. Slots per bucket: {:.1}",
		used_buckets,
		100.0 * (used_buckets as f32 / blen as f32),
		n_slots as f32 / used_buckets as f32
	);
	info!(
		"Data: indexed: {}, hash collisions {:.2} %",
		n_slots,
		(1.0 - (roots.len() as f32) / (n_slots as f32)) * 100.0
	);

	let mut indexed_garbage = 0;
	let referred_garbage = 0;
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
	if !roots.is_empty() {
		panic!("ERROR {} roots point to non-existent data", roots.len());
	}
	info!("Referred: {}", referred);
	info!(
		"Garbage: indexed: {}, referred: {}, links: {}",
		indexed_garbage,
		referred_garbage,
		n_links - used_buckets
	);
}

fn hash(key: &[u8], sip0: u64, sip1: u64) -> u32 {
	siphash24::Hash::hash_to_u64_with_keys(sip0, sip1, key) as u32
}

#[test]
fn test_stats() {
	let mut db = super::api::transient().unwrap();
	db.put_keyed(&[5], &[5]).unwrap();
	db.batch().unwrap();
	db.stats();
}
