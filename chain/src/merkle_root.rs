use bitcrypto::Hash;
use bitcrypto::SHA256D;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

#[inline]
fn concat(a: &SHA256D, b: &SHA256D) -> [u8; 64] {
	let mut result = [0; 64];
	result[0..32].copy_from_slice(a as &[u8]);
	result[32..64].copy_from_slice(b as &[u8]);
	result
}

/// Calculates the root of the merkle tree
/// https://en.bitcoin.it/wiki/Protocol_documentation#Merkle_Trees
pub fn merkle_root(hashes: &[SHA256D]) -> SHA256D {
	if hashes.len() == 1 {
		return hashes[0];
	}
	let mut row = Vec::with_capacity(hashes.len() / 2);
	let mut i = 0;
	while i + 1 < hashes.len() {
		row.push((&hashes[i], &hashes[i + 1]));
		i += 2
	}

	// duplicate the last element if len is not even
	if hashes.len() % 2 == 1 {
		let last = &hashes[hashes.len() - 1];
		row.push((last, last));
	}
	let res: Vec<_>;
	// Only compute in parallel if there is enough work to benefit it
	if row.len() > 250 {
		res = row.par_iter().map(|x| merkle_node_hash(&x.0, &x.1)).collect();
	} else {
		res = row.iter().map(|x| merkle_node_hash(&x.0, &x.1)).collect();
	}
	merkle_root(&res)
}

/// Calculate merkle tree node hash
pub fn merkle_node_hash(left: &SHA256D, right: &SHA256D) -> SHA256D {
	SHA256D::hash(&concat(left, right))
}

#[cfg(test)]
mod tests {
	use super::merkle_root;
	use bitcrypto::{FromStr, SHA256D};

	// block 80_000
	// https://blockchain.info/block/000000000043a8c0fd1d6f726790caa2a406010d19efd2780db27bdbbd93baf6
	#[test]
	fn test_merkle_root_with_2_hashes() {
		let tx1 = SHA256D::from_str("c06fbab289f723c6261d3030ddb6be121f7d2508d77862bb1e484f5cd7f92b25").unwrap();
		let tx2 = SHA256D::from_str("5a4ebf66822b0b2d56bd9dc64ece0bc38ee7844a23ff1d7320a88c5fdb2ad3e2").unwrap();
		let expected = SHA256D::from_str("8fb300e3fdb6f30a4c67233b997f99fdd518b968b9a3fd65857bfe78b2600719").unwrap();

		let result = merkle_root(&[tx1, tx2]);
		let result2 = merkle_root(&[tx1, tx2]);
		assert_eq!(result, expected);
		assert_eq!(result2, expected);
	}

	// Test with 5 hashes
	#[test]
	fn test_merkle_root_with_5_hashes() {
		let mut vec = Vec::new();
		vec.push(SHA256D::from_str("1da63abbc8cc611334a753c4c31de14d19839c65b2b284202eaf3165861fb58d").unwrap());
		vec.push(SHA256D::from_str("26c6a6f18d13d2f0787c1c0f3c5e23cf5bc8b3de685dd1923ae99f44c5341c0c").unwrap());
		vec.push(SHA256D::from_str("513507fa209db823541caf7b9742bb9999b4a399cf604ba8da7037f3acced649").unwrap());
		vec.push(SHA256D::from_str("6bf5d2e02b8432d825c5dff692d435b6c5f685d94efa6b3d8fb818f2ecdcfb66").unwrap());
		vec.push(SHA256D::from_str("8a5ad423bc54fb7c76718371fd5a73b8c42bf27beaf2ad448761b13bcafb8895").unwrap());
		let result = merkle_root(&vec[..]);

		let expected = SHA256D::from_str("3a432cd416ea05b1be4ec1e72d7952d08670eaa5505b6794a186ddb253aa62e6").unwrap();
		assert_eq!(result, expected);
	}
}
