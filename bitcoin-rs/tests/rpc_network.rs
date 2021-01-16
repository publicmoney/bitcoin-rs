use node_manager::node;
use node_manager::NodeManager;

#[tokio::test]
async fn test_rpc_network() {
	let mut bitcoin_rs_1 = node!(1);
	bitcoin_rs_1.start().connect_rpc().await;

	let network_info = bitcoin_rs_1.rpc().network_info().await.unwrap();
	assert_eq!(0, network_info.connections);
	assert!(network_info.networkactive);

	let mut bitcoin_rs_2 = node!(2);
	bitcoin_rs_2.start().connect_rpc().await;

	bitcoin_rs_1.connect_node(&bitcoin_rs_2).await;
	let connections = bitcoin_rs_1.rpc().connection_count().await.unwrap();
	assert_eq!(1, connections);

	let node_info = bitcoin_rs_1.rpc().node_info(false, None).await.unwrap();
	assert_eq!(1, node_info.len());

	let peers_1 = bitcoin_rs_1.rpc().peer_info().await.unwrap();
	assert_eq!(1, peers_1.len());
	assert!(!peers_1.get(0).unwrap().inbound);

	let peers_2 = bitcoin_rs_2.rpc().peer_info().await.unwrap();
	assert_eq!(1, peers_2.len());
	assert!(peers_2.get(0).unwrap().inbound);
}
