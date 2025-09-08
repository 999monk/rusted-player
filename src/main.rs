use std::{env, io};

mod models;
mod services;

pub use services::config_service;
use services::ui_manager;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // Si se da un argumento se establece como path
    if args.len() > 1 {
        let music_path = args[1].clone();
        if std::path::Path::new(&music_path).is_dir() {
            let config = models::config::Config { music_path };
            config_service::save_config(&config)?;
            println!("Music path set to '{}' and saved.", config.music_path);
            ui_manager::run(&config)?;
        } else {
            println!("Error: The provided path is not a valid directory.");
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
        }
        return Ok(());
    }

    // si no, se carga desde cfg el path establecido
    let config = config_service::load_config();

    if config.music_path.is_empty() {
        println!("Music path is not set.");
        println!("Please run the application with the path to your music directory as an argument:");
        println!(r#"Example: rusted-player.exe "C:\Users\YourUser\Music""#);
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        return Ok(());
    }

    if !std::path::Path::new(&config.music_path).is_dir() {
        println!("Error: The saved music path is not a valid directory.");
        println!("Please run the application with a valid path to update it.");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        return Ok(());
    }

    ui_manager::run(&config)?;

    Ok(())
}
