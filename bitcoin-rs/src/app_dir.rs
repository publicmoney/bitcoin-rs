use crate::config::USER_AGENT;
use app_dirs::{app_dir, AppDataType, AppInfo};
use std::fs::create_dir_all;
use std::path::PathBuf;

pub const APP_INFO: AppInfo = AppInfo {
	name: USER_AGENT,
	author: "publicmoney",
};

pub fn app_path(data_dir: &Option<String>, sub_dir: &str) -> String {
	let db_path = match *data_dir {
		Some(ref data_dir) => {
			let mut path = PathBuf::from(data_dir);
			path.push(sub_dir);
			create_dir_all(&path).expect("Failed to create app dir");
			path
		}
		None => app_dir(AppDataType::UserData, &APP_INFO, &sub_dir).expect("Failed to get app dir"),
	};
	db_path.to_str().unwrap().to_string()
}
