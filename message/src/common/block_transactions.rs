use bitcrypto::SHA256D;
use chain::Transaction;

#[derive(Debug, PartialEq, Serializable, Deserializable)]
pub struct BlockTransactions {
	pub blockhash: SHA256D,
	pub transactions: Vec<Transaction>,
}
