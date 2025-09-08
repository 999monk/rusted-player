use std::fs;
use std::env;
use crate::models::config::Config;

pub fn load_config() -> Config {
    let exe_path = env::current_exe().unwrap();
    let exe_dir = exe_path.parent().unwrap();
    let config_path = exe_dir.join("config.json");

    let config = match fs::read_to_string(config_path) {
        Ok(config_str) => {
            let mut config: Config = serde_json::from_str(&config_str).unwrap_or_else(|_| Config {
                music_path: String::new(),
            });

            if config.music_path.contains("%USERNAME%") {
                if let Ok(username) = env::var("USERNAME") {
                    config.music_path = config.music_path.replace("%USERNAME%", &username);
                }
            }
            config
        }
        Err(_) => Config {
            music_path: String::new(),
        },
    };

    config
}

pub fn save_config(config: &Config) -> Result<(), std::io::Error> {
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path.parent().unwrap();
    let config_path = exe_dir.join("config.json");
    let config_str = serde_json::to_string_pretty(config)?;
    fs::write(config_path, config_str)?;
    Ok(())
}
