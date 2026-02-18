use crate::models::config::Config;
use std::env;
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR_NAME: &str = "rusted-player";
const CONFIG_FILE_NAME: &str = "config.json";

/// Obtiene el directorio de configuración del usuario
///
/// Soporta:
/// - Linux/Unix: XDG_CONFIG_HOME o ~/.config/
/// - macOS: ~/Library/Application Support/ (estándar macOS) o ~/.config/
/// - Windows: %APPDATA%\rusted-player\
fn get_config_dir() -> Result<PathBuf, std::io::Error> {
    // Prioridad 1: XDG_CONFIG_HOME (Linux con XDG)
    if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
        let dir = PathBuf::from(xdg_config).join(CONFIG_DIR_NAME);
        ensure_dir_exists(&dir)?;
        return Ok(dir);
    }

    // Prioridad 2: APPDATA (Windows)
    if let Ok(appdata) = env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join(CONFIG_DIR_NAME);
        ensure_dir_exists(&dir)?;
        return Ok(dir);
    }

    // Prioridad 3: HOME (Linux/macOS/Unix)
    if let Ok(home) = env::var("HOME") {
        // En macOS, usar Application Support es más estándar
        #[cfg(target_os = "macos")]
        let dir = PathBuf::from(&home)
            .join("Library")
            .join("Application Support")
            .join(CONFIG_DIR_NAME);
        #[cfg(not(target_os = "macos"))]
        let dir = PathBuf::from(&home).join(".config").join(CONFIG_DIR_NAME);

        ensure_dir_exists(&dir)?;
        return Ok(dir);
    }

    // Prioridad 4: USERPROFILE (Windows fallback)
    if let Ok(userprofile) = env::var("USERPROFILE") {
        let dir = PathBuf::from(userprofile)
            .join("AppData")
            .join("Roaming")
            .join(CONFIG_DIR_NAME);
        ensure_dir_exists(&dir)?;
        return Ok(dir);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "No se pudo determinar el directorio de configuración del usuario",
    ))
}

/// Helper para crear directorio si no existe
fn ensure_dir_exists(dir: &PathBuf) -> Result<(), std::io::Error> {
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Obtiene la ruta al archivo de configuración
fn get_config_path() -> Result<PathBuf, std::io::Error> {
    Ok(get_config_dir()?.join(CONFIG_FILE_NAME))
}

/// Expande variables de entorno en la ruta
///
/// Soporta:
/// - Windows: %USERNAME%, %USERPROFILE%, %APPDATA%, %LOCALAPPDATA%
/// - Linux/macOS: $USER, $HOME, ~
fn expand_env_vars(path: &str) -> PathBuf {
    let mut result = path.to_string();

    // Windows variables
    if result.contains("%USERNAME%") {
        if let Ok(username) = env::var("USERNAME") {
            result = result.replace("%USERNAME%", &username);
        }
    }
    if result.contains("%USERPROFILE%") {
        if let Ok(userprofile) = env::var("USERPROFILE") {
            result = result.replace("%USERPROFILE%", &userprofile);
        }
    }
    if result.contains("%APPDATA%") {
        if let Ok(appdata) = env::var("APPDATA") {
            result = result.replace("%APPDATA%", &appdata);
        }
    }
    if result.contains("%LOCALAPPDATA%") {
        if let Ok(localappdata) = env::var("LOCALAPPDATA") {
            result = result.replace("%LOCALAPPDATA%", &localappdata);
        }
    }

    // Unix/Linux/macOS variables
    if result.contains("$USER") {
        if let Ok(user) = env::var("USER") {
            result = result.replace("$USER", &user);
        }
    }
    if result.contains("$HOME") || result.contains("~") {
        if let Ok(home) = env::var("HOME") {
            result = result.replace("$HOME", &home);
            result = result.replace("~", &home);
        }
    }

    PathBuf::from(result)
}

/// Carga la configuración desde el archivo config.json
///
/// Busca en:
/// - Linux: ~/.config/rusted-player/config.json
/// - macOS: ~/Library/Application Support/rusted-player/config.json
/// - Windows: %APPDATA%\rusted-player\config.json
///
/// Si el archivo no existe o no se puede leer, retorna configuración por defecto.
pub fn load_config() -> Config {
    let config_path = match get_config_path() {
        Ok(path) => path,
        Err(_e) => {
            return Config::default();
        }
    };

    let config = match fs::read_to_string(&config_path) {
        Ok(config_str) => {
            let mut config: Config = match serde_json::from_str(&config_str) {
                Ok(cfg) => cfg,
                Err(_e) => Config::default(),
            };

            // Expande variables de entorno en la ruta
            if let Some(path_str) = config.music_path.to_str() {
                config.music_path = expand_env_vars(path_str);
            }
            config
        }
        Err(_e) => {
            // Archivo no existe o error de lectura - usar configuración por defecto
            Config::default()
        }
    };

    config
}

/// Guarda la configuración en el archivo config.json
///
/// Guarda en la ubicación apropiada según el sistema operativo.
/// Crea el directorio si no existe.
pub fn save_config(config: &Config) -> Result<(), std::io::Error> {
    let config_path = get_config_path()?;
    let config_str = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(config_path, config_str)?;
    Ok(())
}
