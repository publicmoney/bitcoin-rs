use node_manager::node;
use node_manager::NodeManager;

#[tokio::test]
async fn test_rpc_getmemoryinfo() {
	let mut bitcoin_rs = node!();
	bitcoin_rs.start().connect_rpc().await;

	let memory_info = bitcoin_rs.rpc().get_memory_info().await.unwrap();

	assert!(memory_info.free > 0);
	assert!(memory_info.total > 0);
	assert!(memory_info.used > 0);
}
