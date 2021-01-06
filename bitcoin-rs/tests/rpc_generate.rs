use node_manager::node;
use node_manager::NodeManager;

#[tokio::test]
async fn test_rpc_generate() {
	let mut bitcoin_rs = node!();
	bitcoin_rs.start().connect_rpc().await;
	let n_blocks = 2;

	let hashes = bitcoin_rs.rpc().generate(n_blocks, None).await.unwrap();

	assert_eq!(n_blocks, hashes.len() as u32);
	assert_eq!(n_blocks, bitcoin_rs.rpc().block_count().await.unwrap());
}
