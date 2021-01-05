use node_manager::node;
use node_manager::NodeManager;
use std::time::Duration;

#[tokio::test]
async fn test_rpc_stop() {
	let mut bitcoin_rs = node!();
	bitcoin_rs.start().connect_rpc().await;

	bitcoin_rs.rpc().stop().await.unwrap();

	let exit_status = bitcoin_rs.wait_for_exit(Duration::from_secs(10)).await.unwrap();
	assert!(exit_status.success());
}
