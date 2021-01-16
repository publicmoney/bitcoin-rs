use node_manager::node;
use node_manager::NodeManager;
use std::time::Duration;

#[tokio::test]
async fn test_rpc_control() {
	let mut bitcoin_rs = node!();
	bitcoin_rs.start().connect_rpc().await;

	let memory_info = bitcoin_rs.rpc().memory_info().await.unwrap();
	assert!(memory_info.free > 0);
	assert!(memory_info.total > 0);
	assert!(memory_info.used > 0);

	tokio::time::sleep(Duration::from_millis(100)).await;
	let uptime = bitcoin_rs.rpc().uptime().await.unwrap();
	assert!(uptime > 0);

	bitcoin_rs.rpc().stop().await.unwrap();

	let exit_status = bitcoin_rs.wait_for_exit(Duration::from_secs(10)).await.unwrap();
	assert!(exit_status.success());
}
