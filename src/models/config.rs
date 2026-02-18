use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Estructura de configuración del reproductor de música
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    /// Ruta al directorio principal de música
    pub music_path: PathBuf,
}
