use bitcrypto::SHA256D;
use linked_hash_map::LinkedHashMap;

/// Maximal number of hashes to store in known-hashes filter
pub const MAX_KNOWN_HASHES_LEN: usize = 2048;

/// Hash-knowledge type
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum KnownHashType {
	/// Peer knows transaction with this hash
	Transaction,
	/// Peer knows block with this hash
	Block,
	/// Peer knows compact block with this hash
	CompactBlock,
}

/// Known-hashes filter
#[derive(Debug, Default)]
pub struct KnownHashFilter {
	/// Insertion-time ordered known hashes
	known_hashes: LinkedHashMap<SHA256D, KnownHashType>,
}

impl KnownHashFilter {
	/// Insert known hash
	pub fn insert(&mut self, hash: SHA256D, hash_type: KnownHashType) {
		if !self.known_hashes.contains_key(&hash) {
			self.known_hashes.insert(hash, hash_type);
			// remove oldest-known hash, if limits overflow
			if self.known_hashes.len() > MAX_KNOWN_HASHES_LEN {
				self.known_hashes.pop_front();
			}
		}
	}

	/// Returns number of known hashes
	#[cfg(test)]
	pub fn len(&self) -> usize {
		self.known_hashes.len()
	}

	/// Returns true if peer knows about this hash with this type
	pub fn contains(&self, hash: &SHA256D, hash_type: KnownHashType) -> bool {
		self.known_hashes
			.get(hash)
			.map(|stored_hash_type| *stored_hash_type == hash_type)
			.unwrap_or(false)
	}

	/// Filter block using its hash
	pub fn filter_block(&self, hash: &SHA256D) -> bool {
		self.known_hashes
			.get(hash)
			.map(|stored_hash_type| *stored_hash_type != KnownHashType::Block && *stored_hash_type != KnownHashType::CompactBlock)
			.unwrap_or(true)
	}

	/// Filter transaction using its hash
	pub fn filter_transaction(&self, hash: &SHA256D) -> bool {
		self.known_hashes
			.get(hash)
			.map(|stored_hash_type| *stored_hash_type != KnownHashType::Transaction)
			.unwrap_or(true)
	}
}

#[cfg(test)]
mod tests {
	use super::{KnownHashFilter, KnownHashType, MAX_KNOWN_HASHES_LEN};
	use bitcrypto::{FromStr, Hash, SHA256D};

	#[test]
	fn known_hash_filter_empty() {
		assert!(KnownHashFilter::default().filter_transaction(&SHA256D::default()));
		assert!(KnownHashFilter::default().filter_block(&SHA256D::default()));
	}

	#[test]
	fn known_hash_filter_block() {
		let mut filter = KnownHashFilter::default();
		filter.insert(SHA256D::default(), KnownHashType::Block);
		filter.insert(
			SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
			KnownHashType::CompactBlock,
		);
		filter.insert(
			SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap(),
			KnownHashType::Transaction,
		);
		assert!(!filter.filter_block(&SHA256D::default()));
		assert!(!filter.filter_block(&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()));
		assert!(filter.filter_block(&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap()));
		assert!(filter.filter_block(&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap()));
	}

	#[test]
	fn known_hash_filter_transaction() {
		let mut filter = KnownHashFilter::default();
		filter.insert(SHA256D::default(), KnownHashType::Block);
		filter.insert(
			SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
			KnownHashType::CompactBlock,
		);
		filter.insert(
			SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap(),
			KnownHashType::Transaction,
		);
		assert!(filter.filter_transaction(&SHA256D::default()));
		assert!(filter.filter_transaction(&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()));
		assert!(!filter.filter_transaction(&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap()));
		assert!(filter.filter_transaction(&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap()));
	}

	#[test]
	fn known_hash_filter_contains() {
		let mut filter = KnownHashFilter::default();
		filter.insert(SHA256D::default(), KnownHashType::Block);
		filter.insert(
			SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
			KnownHashType::CompactBlock,
		);
		filter.insert(
			SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap(),
			KnownHashType::Transaction,
		);
		assert!(filter.contains(&SHA256D::default(), KnownHashType::Block));
		assert!(!filter.contains(&SHA256D::default(), KnownHashType::CompactBlock));
		assert!(filter.contains(
			&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
			KnownHashType::CompactBlock
		));
		assert!(!filter.contains(
			&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
			KnownHashType::Block
		));
		assert!(filter.contains(
			&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap(),
			KnownHashType::Transaction
		));
		assert!(!filter.contains(
			&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap(),
			KnownHashType::Block
		));
		assert!(!filter.contains(
			&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap(),
			KnownHashType::Block
		));
		assert!(!filter.contains(
			&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap(),
			KnownHashType::CompactBlock
		));
		assert!(!filter.contains(
			&SHA256D::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap(),
			KnownHashType::Transaction
		));
	}

	#[test]
	fn known_hash_filter_insert() {
		let mut hash_data = [0u8; 32];
		let mut filter = KnownHashFilter::default();
		assert_eq!(filter.len(), 0);
		// insert new hash
		filter.insert(SHA256D::from_slice(&hash_data).unwrap(), KnownHashType::Block);
		assert_eq!(filter.len(), 1);
		// insert already known hash => nothing should change
		filter.insert(SHA256D::from_slice(&hash_data).unwrap(), KnownHashType::Block);
		assert_eq!(filter.len(), 1);
		// insert MAX_KNOWN_HASHES_LEN
		for i in 1..MAX_KNOWN_HASHES_LEN {
			hash_data[0] = (i % 255) as u8;
			hash_data[1] = ((i / 255) % 255) as u8;
			filter.insert(SHA256D::from_slice(&hash_data).unwrap(), KnownHashType::Block);
			assert_eq!(filter.len(), i + 1);
		}
		// insert new unknown hash => nothing should change as we already have max number of hashes
		hash_data[0] = ((MAX_KNOWN_HASHES_LEN + 1) % 255) as u8;
		hash_data[1] = (((MAX_KNOWN_HASHES_LEN + 1) / 255) % 255) as u8;
		filter.insert(SHA256D::from_slice(&hash_data).unwrap(), KnownHashType::Block);
		assert_eq!(filter.len(), MAX_KNOWN_HASHES_LEN);
		// check that oldest known hash has been removed
		hash_data[0] = 0;
		hash_data[1] = 0;
		assert!(!filter.contains(&SHA256D::from_slice(&hash_data).unwrap(), KnownHashType::Block));
		hash_data[0] = 1;
		hash_data[1] = 0;
		assert!(filter.contains(&SHA256D::from_slice(&hash_data).unwrap(), KnownHashType::Block));
	}
}
