use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

const PLAYLISTS_SUBDIR: &str = "playlists";
const PLAYLIST_EXTENSION: &str = "json";
const CONFIG_DIR_NAME: &str = "rusted-player";

/// Obtiene el directorio de configuración del usuario
///
/// Soporta:
/// - Linux/Unix: XDG_CONFIG_HOME o ~/.config/
/// - macOS: ~/Library/Application Support/
/// - Windows: %APPDATA%\rusted-player\
fn get_config_dir() -> Result<PathBuf, PlaylistError> {
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

    Err(PlaylistError::InvalidName(
        "No se pudo determinar el directorio de configuración".to_string(),
    ))
}

/// Helper para crear directorio si no existe
fn ensure_dir_exists(dir: &PathBuf) -> Result<(), PlaylistError> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Obtiene el directorio donde se guardan las playlists
fn playlists_dir() -> Result<PathBuf, PlaylistError> {
    let playlists_dir = get_config_dir()?.join(PLAYLISTS_SUBDIR);

    // Crea el subdirectorio si no existe
    if !playlists_dir.exists() {
        std::fs::create_dir_all(&playlists_dir)?;
    }

    Ok(playlists_dir)
}

/// Errores posibles al trabajar con playlists
#[derive(Debug)]
pub enum PlaylistError {
    Io(std::io::Error),
    Serialization(serde_json::Error),
    InvalidName(String),
}

impl std::fmt::Display for PlaylistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Error de E/S: {}", e),
            Self::Serialization(e) => write!(f, "Error de serialización: {}", e),
            Self::InvalidName(s) => write!(f, "Nombre de playlist inválido: {}", s),
        }
    }
}

impl std::error::Error for PlaylistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Serialization(e) => Some(e),
            Self::InvalidName(_) => None,
        }
    }
}

impl From<std::io::Error> for PlaylistError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for PlaylistError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e)
    }
}

/// Representa una lista de reproducción con nombre y pistas
#[derive(Serialize, Deserialize, Debug)]
pub struct Playlist {
    /// Nombre de la lista (usado como nombre de archivo)
    pub name: String,
    /// Rutas a los archivos de audio
    pub tracks: Vec<PathBuf>,
}

/// Valida que el nombre de playlist sea seguro para usar como nombre de archivo
///
/// No permite: nombres vacíos, caracteres de path (/, \, :), ni caracteres
/// inválidos en nombres de archivo (*, ?, ", <, >, |)
fn validate_playlist_name(name: &str) -> Result<(), PlaylistError> {
    if name.is_empty() {
        return Err(PlaylistError::InvalidName(
            "El nombre no puede estar vacío".to_string(),
        ));
    }

    // Caracteres que podrían causar path traversal o son inválidos en nombres de archivo
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    if let Some(ch) = name.chars().find(|c| invalid_chars.contains(c)) {
        return Err(PlaylistError::InvalidName(format!(
            "El nombre contiene caracteres inválidos: '{}'",
            ch
        )));
    }

    // Evitar nombres que podrían ser peligrosos
    let dangerous = [".", "..", "", " "];
    if dangerous.contains(&name) {
        return Err(PlaylistError::InvalidName(format!(
            "Nombre de playlist no permitido: '{}'",
            name
        )));
    }

    Ok(())
}

/// Construye la ruta al archivo de una playlist
fn playlist_file_path(name: &str) -> Result<PathBuf, PlaylistError> {
    validate_playlist_name(name)?;
    Ok(playlists_dir()?.join(format!("{}.json", name)))
}

/// Verifica si un archivo tiene extensión JSON (case-insensitive)
fn is_json_file(path: &PathBuf) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case(PLAYLIST_EXTENSION))
        == Some(true)
}

/// Guarda una playlist en disco como archivo JSON
///
/// Guarda en ~/.config/rusted-player/playlists/
///
/// # Errores
/// - Retorna `InvalidName` si el nombre contiene caracteres inválidos
/// - Retorna `Io` si falla la escritura
/// - Retorna `Serialization` si falla la serialización
pub fn save_playlist(playlist: &Playlist) -> Result<(), PlaylistError> {
    let playlist_path = playlist_file_path(&playlist.name)?;
    let playlist_json = serde_json::to_string_pretty(playlist)?;
    std::fs::write(playlist_path, playlist_json)?;

    Ok(())
}

/// Carga todas las playlists del directorio
///
/// Busca en ~/.config/rusted-player/playlists/
/// Ignora archivos que no sean JSON válidos o que tengan nombres inválidos.
/// Retorna vector vacío si el directorio no existe.
///
/// # Errores
/// - Retorna `Io` si falla la lectura del directorio
pub fn load_playlists() -> Result<Vec<Playlist>, PlaylistError> {
    let playlists_dir = match playlists_dir() {
        Ok(dir) => dir,
        Err(_) => return Ok(Vec::new()),
    };

    if !playlists_dir.exists() {
        return Ok(Vec::new());
    }

    let mut playlists = Vec::new();
    for entry_result in std::fs::read_dir(playlists_dir)? {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_e) => continue,
        };

        let path = entry.path();
        if path.is_file() && is_json_file(&path) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(playlist) = serde_json::from_str::<Playlist>(&content) {
                    playlists.push(playlist);
                }
            }
        }
    }

    Ok(playlists)
}

/// Carga una playlist específica por nombre
///
/// Busca en ~/.config/rusted-player/playlists/
/// Retorna `Ok(None)` si la playlist no existe.
pub fn load_playlist(name: &str) -> Result<Option<Playlist>, PlaylistError> {
    let path = playlist_file_path(name)?;

    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&content)?))
}

/// Elimina una playlist por nombre
///
/// Elimina de ~/.config/rusted-player/playlists/
/// Retorna `true` si se eliminó el archivo, `false` si no existía.
///
/// # Errores
/// - Retorna `InvalidName` si el nombre es inválido
/// - Retorna `Io` para otros errores de E/S
pub fn delete_playlist(playlist_name: &str) -> Result<bool, PlaylistError> {
    let path = playlist_file_path(playlist_name)?;

    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
