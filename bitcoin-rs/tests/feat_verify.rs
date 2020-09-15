extern crate node_manager;
use node_manager::NodeManager;

const BITCOIN_RS: &str = env!("CARGO_BIN_EXE_bitcoin-rs");

#[tokio::test]
async fn test_feat_verify() {
	let mut bitcoin_rs = NodeManager::new_node(BITCOIN_RS);
	bitcoin_rs.builder().with_sub_command("verify".to_string());
	bitcoin_rs.start();

	let exit_status = bitcoin_rs.wait_for_exit().unwrap();
	assert!(exit_status.success());
}
