extern crate node_manager;
use node_manager::NodeManager;
use node_manager::{node, SubCommand};
use tokio::time::Duration;

#[tokio::test]
async fn test_feat_verify() {
	let mut bitcoin_rs = node!();
	bitcoin_rs.with_sub_command(SubCommand::Verify).start();

	let exit_status = bitcoin_rs.wait_for_exit(Duration::from_secs(10)).await.unwrap();
	assert!(exit_status.success());
}
