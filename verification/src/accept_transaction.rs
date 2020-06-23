use crate::canon::CanonTransaction;
use crate::constants::COINBASE_MATURITY;
use crate::deployments::BlockDeployments;
use crate::error::TransactionError;
use crate::network::ConsensusParams;
use crate::primitives::hash::H256;
use crate::script::{verify_script, Script, SignatureVersion, TransactionInputSigner, TransactionSignatureChecker, VerificationFlags};
use crate::sigops::transaction_sigops;
use crate::storage::{DuplexTransactionOutputProvider, TransactionMetaProvider, TransactionOutputProvider};
use crate::VerificationLevel;

pub struct TransactionAcceptor<'a> {
	pub premature_witness: TransactionPrematureWitness<'a>,
	pub bip30: TransactionBip30<'a>,
	pub missing_inputs: TransactionMissingInputs<'a>,
	pub maturity: TransactionMaturity<'a>,
	pub overspent: TransactionOverspent<'a>,
	pub double_spent: TransactionDoubleSpend<'a>,
	pub eval: TransactionEval<'a>,
}

impl<'a> TransactionAcceptor<'a> {
	pub fn new(
		// in case of block validation, it's only current block,
		meta_store: &'a dyn TransactionMetaProvider,
		// previous transaction outputs
		// in case of block validation, that's database and currently processed block
		output_store: DuplexTransactionOutputProvider<'a>,
		consensus: &'a ConsensusParams,
		transaction: CanonTransaction<'a>,
		verification_level: VerificationLevel,
		block_hash: &'a H256,
		height: u32,
		time: u32,
		transaction_index: usize,
		deployments: &'a BlockDeployments<'a>,
	) -> Self {
		trace!(target: "verification", "Tx verification {}", transaction.hash.to_reversed_str());
		TransactionAcceptor {
			premature_witness: TransactionPrematureWitness::new(transaction, deployments),
			bip30: TransactionBip30::new_for_sync(transaction, meta_store, consensus, block_hash, height),
			missing_inputs: TransactionMissingInputs::new(transaction, output_store, transaction_index),
			maturity: TransactionMaturity::new(transaction, meta_store, height),
			overspent: TransactionOverspent::new(transaction, output_store),
			double_spent: TransactionDoubleSpend::new(transaction, output_store),
			eval: TransactionEval::new(transaction, output_store, consensus, verification_level, height, time, deployments),
		}
	}

	pub fn check(&self) -> Result<(), TransactionError> {
		self.premature_witness.check()?;
		self.bip30.check()?;
		self.missing_inputs.check()?;
		self.maturity.check()?;
		self.overspent.check()?;
		self.double_spent.check()?;
		self.eval.check()?;
		Ok(())
	}
}

pub struct MemoryPoolTransactionAcceptor<'a> {
	pub missing_inputs: TransactionMissingInputs<'a>,
	pub maturity: TransactionMaturity<'a>,
	pub overspent: TransactionOverspent<'a>,
	pub sigops: TransactionSigops<'a>,
	pub double_spent: TransactionDoubleSpend<'a>,
	pub eval: TransactionEval<'a>,
}

impl<'a> MemoryPoolTransactionAcceptor<'a> {
	pub fn new(
		// TODO: in case of memory pool it should be db and memory pool
		meta_store: &'a dyn TransactionMetaProvider,
		// in case of memory pool it should be db and memory pool
		output_store: DuplexTransactionOutputProvider<'a>,
		consensus: &'a ConsensusParams,
		transaction: CanonTransaction<'a>,
		height: u32,
		time: u32,
		deployments: &'a BlockDeployments<'a>,
	) -> Self {
		trace!(target: "verification", "Mempool-Tx verification {}", transaction.hash.to_reversed_str());
		let transaction_index = 0;
		MemoryPoolTransactionAcceptor {
			missing_inputs: TransactionMissingInputs::new(transaction, output_store, transaction_index),
			maturity: TransactionMaturity::new(transaction, meta_store, height),
			overspent: TransactionOverspent::new(transaction, output_store),
			sigops: TransactionSigops::new(transaction, output_store, consensus, consensus.max_block_sigops, time),
			double_spent: TransactionDoubleSpend::new(transaction, output_store),
			eval: TransactionEval::new(
				transaction,
				output_store,
				consensus,
				VerificationLevel::Full,
				height,
				time,
				deployments,
			),
		}
	}

	pub fn check(&self) -> Result<(), TransactionError> {
		// Bip30 is not checked because we don't need to allow tx pool acceptance of an unspent duplicate.
		// Tx pool validation is not strinctly a matter of consensus.
		self.missing_inputs.check()?;
		self.maturity.check()?;
		self.overspent.check()?;
		self.sigops.check()?;
		self.double_spent.check()?;
		self.eval.check()?;
		Ok(())
	}
}

/// Bip30 validation
///
/// A transaction hash that exists in the chain is not acceptable even if
/// the original is spent in the new block. This is not necessary nor is it
/// described by BIP30, but it is in the code referenced by BIP30. As such
/// the tx pool need only test against the chain, skipping the pool.
///
/// source:
/// https://github.com/libbitcoin/libbitcoin/blob/61759b2fd66041bcdbc124b2f04ed5ddc20c7312/src/chain/transaction.cpp#L780-L785
pub struct TransactionBip30<'a> {
	transaction: CanonTransaction<'a>,
	store: &'a dyn TransactionMetaProvider,
	exception: bool,
}

impl<'a> TransactionBip30<'a> {
	fn new_for_sync(
		transaction: CanonTransaction<'a>,
		store: &'a dyn TransactionMetaProvider,
		consensus_params: &'a ConsensusParams,
		block_hash: &'a H256,
		height: u32,
	) -> Self {
		let exception = consensus_params.is_bip30_exception(block_hash, height);

		TransactionBip30 {
			transaction,
			store,
			exception,
		}
	}

	fn check(&self) -> Result<(), TransactionError> {
		match self.store.transaction_meta(&self.transaction.hash) {
			Some(ref meta) if !meta.is_fully_spent() && !self.exception => Err(TransactionError::UnspentTransactionWithTheSameHash),
			_ => Ok(()),
		}
	}
}

pub struct TransactionMissingInputs<'a> {
	transaction: CanonTransaction<'a>,
	store: DuplexTransactionOutputProvider<'a>,
	transaction_index: usize,
}

impl<'a> TransactionMissingInputs<'a> {
	fn new(transaction: CanonTransaction<'a>, store: DuplexTransactionOutputProvider<'a>, transaction_index: usize) -> Self {
		TransactionMissingInputs {
			transaction,
			store,
			transaction_index,
		}
	}

	fn check(&self) -> Result<(), TransactionError> {
		let missing_index = self.transaction.raw.inputs.iter().position(|input| {
			let is_not_null = !input.previous_output.is_null();
			let is_missing = self
				.store
				.transaction_output(&input.previous_output, self.transaction_index)
				.is_none();
			is_not_null && is_missing
		});

		match missing_index {
			Some(index) => Err(TransactionError::Input(index)),
			None => Ok(()),
		}
	}
}

pub struct TransactionMaturity<'a> {
	transaction: CanonTransaction<'a>,
	store: &'a dyn TransactionMetaProvider,
	height: u32,
}

impl<'a> TransactionMaturity<'a> {
	fn new(transaction: CanonTransaction<'a>, store: &'a dyn TransactionMetaProvider, height: u32) -> Self {
		TransactionMaturity {
			transaction,
			store,
			height,
		}
	}

	fn check(&self) -> Result<(), TransactionError> {
		// TODO: this is should also fail when we are trying to spend current block coinbase
		let immature_spend =
			self.transaction
				.raw
				.inputs
				.iter()
				.any(|input| match self.store.transaction_meta(&input.previous_output.hash) {
					Some(ref meta) if meta.is_coinbase() && self.height < meta.height() + COINBASE_MATURITY => true,
					_ => false,
				});

		if immature_spend {
			Err(TransactionError::Maturity)
		} else {
			Ok(())
		}
	}
}

pub struct TransactionOverspent<'a> {
	transaction: CanonTransaction<'a>,
	store: DuplexTransactionOutputProvider<'a>,
}

impl<'a> TransactionOverspent<'a> {
	fn new(transaction: CanonTransaction<'a>, store: DuplexTransactionOutputProvider<'a>) -> Self {
		TransactionOverspent { transaction, store }
	}

	fn check(&self) -> Result<(), TransactionError> {
		if self.transaction.raw.is_coinbase() {
			return Ok(());
		}

		let available = self
			.transaction
			.raw
			.inputs
			.iter()
			.map(|input| {
				self.store
					.transaction_output(&input.previous_output, usize::max_value())
					.map(|o| o.value)
					.unwrap_or(0)
			})
			.sum::<u64>();

		let spends = self.transaction.raw.total_spends();

		if spends > available {
			Err(TransactionError::Overspend)
		} else {
			Ok(())
		}
	}
}

pub struct TransactionSigops<'a> {
	transaction: CanonTransaction<'a>,
	store: DuplexTransactionOutputProvider<'a>,
	consensus_params: &'a ConsensusParams,
	max_sigops: usize,
	time: u32,
}

impl<'a> TransactionSigops<'a> {
	fn new(
		transaction: CanonTransaction<'a>,
		store: DuplexTransactionOutputProvider<'a>,
		consensus_params: &'a ConsensusParams,
		max_sigops: usize,
		time: u32,
	) -> Self {
		TransactionSigops {
			transaction,
			store,
			consensus_params,
			max_sigops,
			time,
		}
	}

	fn check(&self) -> Result<(), TransactionError> {
		let bip16_active = self.time >= self.consensus_params.bip16_time;
		let sigops = transaction_sigops(&self.transaction.raw, &self.store, bip16_active);
		if sigops > self.max_sigops {
			Err(TransactionError::MaxSigops)
		} else {
			Ok(())
		}
	}
}

pub struct TransactionEval<'a> {
	transaction: CanonTransaction<'a>,
	store: DuplexTransactionOutputProvider<'a>,
	verification_level: VerificationLevel,
	verify_p2sh: bool,
	verify_strictenc: bool,
	verify_locktime: bool,
	verify_checksequence: bool,
	verify_dersig: bool,
	verify_witness: bool,
	verify_nulldummy: bool,
	verify_monolith_opcodes: bool,
	verify_sigpushonly: bool,
	verify_cleanstack: bool,
	signature_version: SignatureVersion,
}

impl<'a> TransactionEval<'a> {
	fn new(
		transaction: CanonTransaction<'a>,
		store: DuplexTransactionOutputProvider<'a>,
		params: &ConsensusParams,
		verification_level: VerificationLevel,
		height: u32,
		time: u32,
		deployments: &'a BlockDeployments,
	) -> Self {
		let verify_p2sh = time >= params.bip16_time;
		let verify_strictenc = false;
		let verify_locktime = height >= params.bip65_height;
		let verify_dersig = height >= params.bip66_height;
		let verify_monolith_opcodes = false;
		let signature_version = SignatureVersion::Base;

		let verify_checksequence = deployments.csv();
		let verify_witness = deployments.segwit();
		let verify_nulldummy = verify_witness;
		let verify_sigpushonly = false;
		let verify_cleanstack = false;

		TransactionEval {
			transaction,
			store,
			verification_level,
			verify_p2sh,
			verify_strictenc,
			verify_locktime,
			verify_checksequence,
			verify_dersig,
			verify_witness,
			verify_nulldummy,
			verify_monolith_opcodes,
			verify_sigpushonly,
			verify_cleanstack,
			signature_version,
		}
	}

	fn check(&self) -> Result<(), TransactionError> {
		if self.verification_level == VerificationLevel::Header || self.verification_level == VerificationLevel::NoVerification {
			return Ok(());
		}

		if self.transaction.raw.is_coinbase() {
			return Ok(());
		}

		let signer: TransactionInputSigner = self.transaction.raw.clone().into();

		let mut checker = TransactionSignatureChecker {
			signer,
			input_index: 0,
			input_amount: 0,
		};

		for (index, input) in self.transaction.raw.inputs.iter().enumerate() {
			let output = self
				.store
				.transaction_output(&input.previous_output, usize::max_value())
				.ok_or_else(|| TransactionError::UnknownReference(input.previous_output.hash.clone()))?;

			checker.input_index = index;
			checker.input_amount = output.value;

			let script_witness = &input.script_witness;
			let input: Script = input.script_sig.clone().into();
			let output: Script = output.script_pubkey.into();

			let flags = VerificationFlags::default()
				.verify_p2sh(self.verify_p2sh)
				.verify_strictenc(self.verify_strictenc)
				.verify_locktime(self.verify_locktime)
				.verify_checksequence(self.verify_checksequence)
				.verify_dersig(self.verify_dersig)
				.verify_nulldummy(self.verify_nulldummy)
				.verify_witness(self.verify_witness)
				.verify_concat(self.verify_monolith_opcodes)
				.verify_split(self.verify_monolith_opcodes)
				.verify_and(self.verify_monolith_opcodes)
				.verify_or(self.verify_monolith_opcodes)
				.verify_xor(self.verify_monolith_opcodes)
				.verify_div(self.verify_monolith_opcodes)
				.verify_mod(self.verify_monolith_opcodes)
				.verify_bin2num(self.verify_monolith_opcodes)
				.verify_num2bin(self.verify_monolith_opcodes)
				.verify_sigpushonly(self.verify_sigpushonly)
				.verify_cleanstack(self.verify_cleanstack);

			verify_script(&input, &output, &script_witness, &flags, &checker, self.signature_version)
				.map_err(|e| TransactionError::Signature(index, e))?;
		}

		Ok(())
	}
}

pub struct TransactionDoubleSpend<'a> {
	transaction: CanonTransaction<'a>,
	store: DuplexTransactionOutputProvider<'a>,
}

impl<'a> TransactionDoubleSpend<'a> {
	fn new(transaction: CanonTransaction<'a>, store: DuplexTransactionOutputProvider<'a>) -> Self {
		TransactionDoubleSpend { transaction, store }
	}

	fn check(&self) -> Result<(), TransactionError> {
		if self.transaction.raw.is_coinbase() {
			return Ok(());
		}

		for input in &self.transaction.raw.inputs {
			if self.store.is_spent(&input.previous_output) {
				return Err(TransactionError::UsingSpentOutput(
					input.previous_output.hash.clone(),
					input.previous_output.index,
				));
			}
		}
		Ok(())
	}
}

pub struct TransactionPrematureWitness<'a> {
	transaction: CanonTransaction<'a>,
	segwit_active: bool,
}

impl<'a> TransactionPrematureWitness<'a> {
	fn new(transaction: CanonTransaction<'a>, deployments: &'a BlockDeployments<'a>) -> Self {
		let segwit_active = deployments.segwit();

		TransactionPrematureWitness {
			transaction,
			segwit_active,
		}
	}

	fn check(&self) -> Result<(), TransactionError> {
		if !self.segwit_active && (*self.transaction).raw.has_witness() {
			Err(TransactionError::PrematureWitness)
		} else {
			Ok(())
		}
	}
}
