use message::common::InventoryVector;
use message::types::GetData;
use network::Network;
use node_manager::NodeManager;

#[path = "node_manager.rs"]
mod node_manager;

#[tokio::test]
async fn test_p2p_sendheaders() {
	let bitcoin_rs = NodeManager::new_node().await;
	let get_data = GetData::with_inventory(vec![InventoryVector::block(*Network::Regtest.genesis_block().hash())]);

	bitcoin_rs.send_message(&get_data).await;
	let result = bitcoin_rs.read_message().await;

	assert_eq!("getheaders".to_string(), result.0.to_string());
}
