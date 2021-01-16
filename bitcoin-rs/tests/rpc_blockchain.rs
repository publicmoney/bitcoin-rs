use node_manager::node;
use node_manager::NodeManager;
use rpc_server::v1::types::{GetBlockResponse, U256};

#[tokio::test]
async fn test_rpc_blockchain() {
	let mut bitcoin_rs = node!();
	bitcoin_rs.start().connect_rpc().await;
	let hashes = bitcoin_rs.rpc().generate(3, None).await.unwrap();

	let blockchain_info = bitcoin_rs.rpc().blockchain_info().await.unwrap();

	assert_eq!(3, blockchain_info.headers);
	assert_eq!(3, blockchain_info.blocks);
	assert_eq!("other", blockchain_info.chain);
	assert_eq!(hashes.last().cloned(), blockchain_info.bestblockhash);
	assert_eq!(blockchain_info.chainwork, U256::default());
	assert_eq!(0.0, blockchain_info.verificationprogress);
	assert_eq!("", blockchain_info.warnings);
	assert!(blockchain_info.difficulty.unwrap() > 0.0);
	assert!(!blockchain_info.initialblockdownload);
	assert!(!blockchain_info.pruned);
	assert!(blockchain_info.size_on_disk > 0);

	let best_hash = bitcoin_rs.rpc().best_block_hash().await.unwrap();
	assert_eq!(hashes.last().unwrap(), &best_hash);

	let block_count = bitcoin_rs.rpc().block_count().await.unwrap();
	assert_eq!(3, block_count);

	let block_hash = bitcoin_rs.rpc().block_hash(3).await.unwrap();
	assert_eq!(hashes.last().unwrap(), &block_hash);

	let difficulty = bitcoin_rs.rpc().difficulty().await.unwrap();
	assert!(difficulty > 0.0);

	let block = match bitcoin_rs.rpc().block(hashes.last().cloned().unwrap(), Some(true)).await.unwrap() {
		GetBlockResponse::Verbose(block) => block,
		_ => panic!("wrong response type"),
	};
	assert_eq!(hashes.last().cloned().unwrap(), block.hash);

	let tx_out_set_info = bitcoin_rs.rpc().transaction_out_set_info().await.unwrap();
	assert_eq!(3, tx_out_set_info.height);
	assert_eq!(4, tx_out_set_info.transactions);
	assert_eq!(hashes.last().unwrap(), &tx_out_set_info.bestblock);
	assert_eq!(tx_out_set_info.total_amount, 20000000000.0);
	assert_eq!(tx_out_set_info.txouts, 4);
	assert_eq!(0, tx_out_set_info.bogosize);
	assert!(tx_out_set_info.disk_size > 0);

	let chain_tx_stats = bitcoin_rs.rpc().chain_tx_stats(Some(3), None).await.unwrap();
	assert_eq!(chain_tx_stats.time, 0);
	assert_eq!(chain_tx_stats.txcount, 4);
	assert_eq!(chain_tx_stats.txrate, 0);
}
