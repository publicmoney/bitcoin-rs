use message::common::InventoryVector;
use message::types::GetData;
use network::Network;
use node_manager::node;
use node_manager::NodeManager;

#[tokio::test]
async fn test_p2p_sendheaders() {
	let mut bitcoin_rs = node!();
	bitcoin_rs.start().connect_p2p().await;

	let get_data = GetData::with_inventory(vec![InventoryVector::block(*Network::Regtest.genesis_block().hash())]);
	bitcoin_rs.send_message(&get_data).await.unwrap();
	let result = bitcoin_rs.read_message().await.unwrap();

	assert_eq!("getheaders".to_string(), result.0.to_string());
}
