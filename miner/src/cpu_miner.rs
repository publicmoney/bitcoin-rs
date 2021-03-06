use crate::block_template::BlockTemplate;
use bitcrypto::dhash256;
use bitcrypto::SHA256D;
use byteorder::{LittleEndian, WriteBytesExt};
use chain::{merkle_root, BlockHeader, IndexedBlock, Transaction, TransactionInput, TransactionOutput};
use keys::AddressHash;
use primitives::bytes::Bytes;
use primitives::compact::Compact;
use primitives::U256;
use script::Builder;
use ser::Stream;
use verification::is_valid_proof_of_work_hash;

/// Instead of serializing `BlockHeader` from scratch over and over again,
/// let's keep it serialized in memory and replace needed bytes
struct BlockHeaderBytes {
	data: Bytes,
}

impl BlockHeaderBytes {
	/// Creates new instance of block header bytes.
	fn new(version: u32, previous_header_hash: SHA256D, bits: Compact) -> Self {
		let merkle_root_hash = SHA256D::default();
		let time = 0u32;
		let nonce = 0u32;

		let mut stream = Stream::default();
		stream
			.append(&version)
			.append(&previous_header_hash)
			.append(&merkle_root_hash)
			.append(&time)
			.append(&bits)
			.append(&nonce);

		BlockHeaderBytes { data: stream.out() }
	}

	/// Set merkle root hash
	fn set_merkle_root_hash(&mut self, hash: &SHA256D) {
		let merkle_bytes: &mut [u8] = &mut self.data[4 + 32..4 + 32 + 32];
		merkle_bytes.copy_from_slice(&**hash);
	}

	/// Set block header time
	fn set_time(&mut self, time: u32) {
		let mut time_bytes: &mut [u8] = &mut self.data[4 + 32 + 32..];
		time_bytes.write_u32::<LittleEndian>(time).unwrap();
	}

	/// Set block header nonce
	fn set_nonce(&mut self, nonce: u32) {
		let mut nonce_bytes: &mut [u8] = &mut self.data[4 + 32 + 32 + 4 + 4..];
		nonce_bytes.write_u32::<LittleEndian>(nonce).unwrap();
	}

	/// Returns block header hash
	fn hash(&self) -> SHA256D {
		dhash256(&self.data)
	}
}

/// This trait should be implemented by coinbase transaction.
pub trait CoinbaseTransactionBuilder {
	/// Should be used to increase number of hash possibilities for miner
	fn set_extranonce(&mut self, extranonce: &[u8]);
	/// Returns transaction hash
	fn hash(&self) -> SHA256D;
	/// Coverts transaction into raw bytes
	fn finish(self) -> Transaction;
}

pub struct P2shCoinbaseTransactionBuilder {
	transaction: Transaction,
}

impl P2shCoinbaseTransactionBuilder {
	pub fn new(hash: &AddressHash, value: u64) -> Self {
		let script_pubkey = Builder::build_p2sh(hash).into();

		let transaction = Transaction {
			version: 0,
			inputs: vec![TransactionInput::coinbase(Bytes::default())],
			outputs: vec![TransactionOutput { value, script_pubkey }],
			lock_time: 0,
		};

		P2shCoinbaseTransactionBuilder { transaction }
	}
}

impl CoinbaseTransactionBuilder for P2shCoinbaseTransactionBuilder {
	fn set_extranonce(&mut self, extranonce: &[u8]) {
		self.transaction.inputs[0].script_sig = extranonce.to_vec().into();
	}

	fn hash(&self) -> SHA256D {
		self.transaction.hash()
	}

	fn finish(self) -> Transaction {
		self.transaction
	}
}

/// Cpu miner solution.
pub struct Solution {
	/// Block header nonce.
	pub nonce: u32,
	/// Coinbase transaction extra nonce (modifiable by miner).
	pub extranonce: U256,
	/// Block header time.
	pub time: u32,
	/// Coinbase transaction (extranonce is already set).
	pub coinbase_transaction: Transaction,
}

/// Simple bitcoin cpu miner.
///
/// First it tries to find solution by changing block header nonce.
/// Once all nonce values have been tried, it increases extranonce.
/// Once all of them have been tried (quite unlikely on cpu ;),
/// and solution still hasn't been found it returns None.
/// It's possible to also experiment with time, but I find it pointless
/// to implement on CPU.
pub fn find_solution<T>(
	block: &BlockTemplate,
	mut coinbase_transaction_builder: T,
	max_extranonce: U256,
	max_tries: usize,
) -> Option<Solution>
where
	T: CoinbaseTransactionBuilder,
{
	let mut extranonce = U256::from(block.height);
	let mut extranonce_bytes = [0u8; 32];
	let mut tries: usize = 0;
	let mut header_bytes = BlockHeaderBytes::new(block.version, block.previous_header_hash.clone(), block.bits);
	// update header with time
	header_bytes.set_time(block.time);

	while extranonce < max_extranonce && tries <= max_tries {
		extranonce.to_little_endian(&mut extranonce_bytes);
		// update coinbase transaction with new extranonce
		coinbase_transaction_builder.set_extranonce(&extranonce_bytes);

		// recalculate merkle root hash
		let coinbase_hash = coinbase_transaction_builder.hash();
		let mut merkle_tree = vec![coinbase_hash];
		merkle_tree.extend(block.transactions.iter().map(|tx| &tx.hash));
		let merkle_root_hash = merkle_root(merkle_tree.as_slice());

		// update header with new merkle root hash
		header_bytes.set_merkle_root_hash(&merkle_root_hash);

		for nonce in 0..(u32::max_value() as u64 + 1) {
			tries += 1;
			// update §
			header_bytes.set_nonce(nonce as u32);
			let hash = header_bytes.hash();
			if is_valid_proof_of_work_hash(block.bits, &hash) {
				let solution = Solution {
					nonce: nonce as u32,
					extranonce,
					time: block.time,
					coinbase_transaction: coinbase_transaction_builder.finish(),
				};

				return Some(solution);
			}
			if tries == max_tries {
				break;
			}
		}

		extranonce = extranonce + 1;
	}

	None
}

pub fn mine_block(mut block_template: BlockTemplate, address: Option<AddressHash>, max_tries: usize) -> Option<IndexedBlock> {
	let hash = address.unwrap_or_default();
	let coinbase_builder = P2shCoinbaseTransactionBuilder::new(&hash, block_template.coinbase_value);
	if let Some(solution) = find_solution(&block_template, coinbase_builder, U256::max_value(), max_tries) {
		block_template.transactions.push(solution.coinbase_transaction.into());
		let hashes: Vec<SHA256D> = block_template.transactions.iter().map(|t| t.hash).collect();
		let merkle_root_hash = if hashes.is_empty() {
			SHA256D::default()
		} else {
			merkle_root(&hashes)
		};

		return Some(IndexedBlock {
			header: BlockHeader {
				version: block_template.version,
				previous_header_hash: block_template.previous_header_hash,
				merkle_root_hash,
				time: block_template.time,
				bits: block_template.bits,
				nonce: solution.nonce,
			}
			.into(),
			transactions: block_template.transactions,
		});
	}
	None
}

#[cfg(test)]
mod tests {
	use super::{find_solution, P2shCoinbaseTransactionBuilder};
	use crate::block_template::BlockTemplate;
	use crate::cpu_miner::mine_block;
	use bitcrypto::SHA256D;
	use keys::AddressHash;
	use primitives::U256;

	#[test]
	fn test_cpu_miner_low_difficulty() {
		let block_template = BlockTemplate {
			version: 0,
			previous_header_hash: SHA256D::default(),
			time: 0,
			bits: U256::max_value().into(),
			height: 0,
			transactions: Vec::new(),
			coinbase_value: 10,
			size_limit: 1000,
			sigop_limit: 100,
		};

		let hash = AddressHash::default();
		let coinbase_builder = P2shCoinbaseTransactionBuilder::new(&hash, 10);
		let solution = find_solution(&block_template, coinbase_builder, U256::max_value(), usize::max_value());
		assert!(solution.is_some());
	}

	#[test]
	fn test_cpu_miner_mine_block() {
		let block_template = BlockTemplate {
			version: 0,
			previous_header_hash: SHA256D::default(),
			time: 0,
			bits: U256::max_value().into(),
			height: 0,
			transactions: Vec::new(),
			coinbase_value: 10,
			size_limit: 1000,
			sigop_limit: 100,
		};

		let block = mine_block(block_template, None, usize::max_value());
		assert!(block.is_some());
	}
}
