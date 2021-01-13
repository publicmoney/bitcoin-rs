extern crate node_manager;
use node_manager::node;
use node_manager::NodeManager;
use tokio::time::Duration;

#[tokio::test]
async fn test_feat_recover() {
	// Generate some blocks
	let mut bitcoin_rs = node!();
	bitcoin_rs.start().connect_rpc().await;
	bitcoin_rs.rpc().generate(3, None).await.unwrap();

	// Clean shutdown via RPC
	bitcoin_rs.rpc().stop().await.unwrap();
	let exit_status = bitcoin_rs.wait_for_exit(Duration::from_secs(10)).await.unwrap();
	assert!(exit_status.success());

	// Check node can be started again
	bitcoin_rs.start().connect_rpc().await;
	assert_eq!(3, bitcoin_rs.rpc().block_count().await.unwrap());

	// Kill the node and check it can be started again
	bitcoin_rs.kill();
	bitcoin_rs.start().connect_rpc().await;
	assert_eq!(3, bitcoin_rs.rpc().block_count().await.unwrap());
}
