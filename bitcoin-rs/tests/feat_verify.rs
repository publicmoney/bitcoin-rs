extern crate node_manager;
use node_manager::NodeManager;

const BITCOIN_RS: &str = env!("CARGO_BIN_EXE_bitcoin-rs");

#[tokio::test]
async fn test_feat_verify() {
	let bitcoin_rs = NodeManager::new_node(BITCOIN_RS, "verify").with_sub_command("verify").start();

	let exit_status = bitcoin_rs.wait_for_exit().unwrap();
	assert!(exit_status.success());
}
