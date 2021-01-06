use node_manager::node;
use node_manager::NodeManager;

#[tokio::test]
async fn test_rpc_getnetworkinfo() {
	let mut bitcoin_rs = node!();
	bitcoin_rs.start().connect_rpc().await;

	let network_info = bitcoin_rs.rpc().network_info().await.unwrap();

	assert_eq!(0, network_info.connections);
	assert!(network_info.networkactive);
}
