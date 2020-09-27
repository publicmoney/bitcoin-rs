use crate::app_dir::app_path;
use crate::config::Config;
use std::fs;

pub fn setup_log(config: &Config) -> Result<(), String> {
	let config_path = app_path(&config.data_dir, "config") + "/log4rs.yaml";
	let log_path = app_path(&config.data_dir, "log") + "/node.log";

	if fs::metadata(&config_path).is_err() {
		fs::write(&config_path, default_config(log_path)).map_err(|e| e.to_string())?
	}

	log4rs::init_file(config_path, Default::default()).map_err(|e| e.to_string())
}

fn default_config(log_path: String) -> String {
	format!(
		r#"refresh_rate: 60 seconds
appenders:
  stdout:
    kind: console
    encoder:
      pattern: "{{d(%Y-%m-%d %H:%M:%S)}} {{h({{l}})}} {{M}} {{m}}{{n}}"
  file:
    kind: rolling_file
    append: true
    path: "{}"
    encoder:
      pattern: "{{d(%Y-%m-%d %H:%M:%S)}} {{l}} {{M}} {{m}}{{n}}"
    policy:
      trigger:
        kind: size
        limit: 100 mb
      roller:
        kind: delete
root:
  level: info
  appenders:
    - stdout
    - file
"#,
		log_path
	)
}
