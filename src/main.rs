use std::{env, io, path::PathBuf};

mod models;
mod services;

use services::config_service;
use services::ui_manager;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // Si se da un argumento se establece como path
    if args.len() > 1 {
        let music_path = PathBuf::from(&args[1]);
        if music_path.is_dir() {
            let config = models::config::Config { music_path };
            config_service::save_config(&config)?;
            println!(
                "Music path set to '{}' and saved.",
                config.music_path.display()
            );
            ui_manager::run(&config)?;
        } else {
            println!("Error: '{}' is not a valid directory.", args[1]);
            println!("Press Enter to exit...");
            let mut _input = String::new();
            io::stdin().read_line(&mut _input)?;
        }
        return Ok(());
    }

    // si no, se carga desde cfg el path establecido
    let config = config_service::load_config();

    if config.music_path.as_os_str().is_empty() {
        println!("Music path is not set.");
        println!(
            "Please run the application with the path to your music directory as an argument:"
        );
        println!(r#"Example: rusted-player.exe "C:\Users\YourUser\Music""#);
        println!(r#"         rusted-player "/home/username/Music""#);
        println!("Press Enter to exit...");
        let mut _input = String::new();
        io::stdin().read_line(&mut _input)?;
        return Ok(());
    }

    if !config.music_path.is_dir() {
        println!(
            "Error: '{}' is not a valid directory.",
            config.music_path.display()
        );
        println!("Please run the application with a valid path to update it.");
        println!("Press Enter to exit...");
        let mut _input = String::new();
        io::stdin().read_line(&mut _input)?;
        return Ok(());
    }

    ui_manager::run(&config)?;

    Ok(())
}
