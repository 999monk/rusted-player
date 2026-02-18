use lofty::{Accessor, AudioFile, Probe, TaggedFileExt};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Formatos de audio soportados
pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "wav", "m4a", "aac", "wma"];

/// Metadata de una pista de audio
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    /// Ruta al archivo
    pub path: PathBuf,
    /// Nombre del álbum
    pub album: Option<String>,
    /// Nombre del artista
    pub artist: Option<String>,
    /// Género musical
    pub genre: Option<String>,
    /// Año de lanzamiento
    pub year: Option<u32>,
    /// Duración de la pista
    pub duration: Option<std::time::Duration>,
}

/// Servicio para gestionar la biblioteca de música y extraer metadata
#[derive(Debug)]
pub struct PlaylistService {
    tracks: Vec<TrackMetadata>,
    genres: HashMap<String, Vec<usize>>,
    artists: HashMap<String, Vec<usize>>,
}

/// Normaliza un género para agrupación (minúsculas, sin caracteres especiales)
fn normalize_genre(genre: &str) -> String {
    genre
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Verifica si una extensión corresponde a un archivo de audio soportado
fn is_audio_file_ext(ext: &str) -> bool {
    SUPPORTED_AUDIO_EXTENSIONS
        .iter()
        .any(|&e| ext.eq_ignore_ascii_case(e))
}

impl PlaylistService {
    /// Crea un nuevo servicio de playlist vacío
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            genres: HashMap::new(),
            artists: HashMap::new(),
        }
    }

    /// Escanea un directorio recursivamente y extrae metadata de archivos de audio
    ///
    /// # Arguments
    /// * `dir_path` - Ruta al directorio a escanear
    ///
    /// # Errors
    /// Retorna error si falla el recorrido del directorio
    pub fn scan_directory(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.clear_database();

        for entry in WalkDir::new(dir_path) {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && self.is_audio_file(path) {
                match self.extract_metadata(path) {
                    Ok(metadata) => {
                        let index = self.tracks.len();

                        // Agrupa por género normalizado
                        if let Some(ref genre) = metadata.genre {
                            let normalized = normalize_genre(genre);
                            // Usa el nombre normalizado como clave para agrupar variantes
                            self.genres.entry(normalized).or_default().push(index);
                        }

                        // Agrupa por artista
                        if let Some(ref artist) = metadata.artist {
                            self.artists.entry(artist.clone()).or_default().push(index);
                        }

                        self.tracks.push(metadata);
                    }
                    Err(_e) => {
                        // Errores de encoding son comunes en MP3, agregamos el archivo sin metadata
                        self.tracks.push(TrackMetadata {
                            path: path.to_path_buf(),
                            album: None,
                            artist: None,
                            genre: None,
                            year: None,
                            duration: None,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Extrae metadata de un archivo de audio
    fn extract_metadata(&self, path: &Path) -> Result<TrackMetadata, Box<dyn std::error::Error>> {
        // Abrir y leer el archivo - lofty maneja internamente la mayoría de errores de encoding
        let tagged_file = Probe::open(path)?.read()?;

        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag());
        let properties = tagged_file.properties();

        let tag_ref = tag.as_ref();

        // lofty ya maneja internamente la conversión de strings, simplemente usamos los valores
        let metadata = TrackMetadata {
            path: path.to_path_buf(),
            album: tag_ref.and_then(|t| t.album().map(|s| s.to_string())),
            artist: tag_ref.and_then(|t| t.artist().map(|s| s.to_string())),
            genre: tag_ref.and_then(|t| t.genre().map(|s| s.to_string())),
            year: tag.and_then(|t| t.year()),
            duration: Some(properties.duration()),
        };
        Ok(metadata)
    }

    /// Genera una playlist con todas las pistas de un género
    pub fn get_playlist_by_genre(&self, genre: &str) -> Vec<PathBuf> {
        let normalized = normalize_genre(genre);
        self.genres
            .get(&normalized)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&i| self.tracks[i].path.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Obtiene lista de géneros únicos
    pub fn get_genres(&self) -> Vec<String> {
        let mut genres: Vec<_> = self.genres.keys().cloned().collect();
        genres.sort();
        genres
    }

    /// Obtiene los 5 artistas con más pistas
    pub fn get_top_artists(&self) -> Vec<(String, usize)> {
        let mut artists: Vec<(String, usize)> = self
            .artists
            .iter()
            .map(|(artist, tracks)| (artist.clone(), tracks.len()))
            .collect();
        artists.sort_by(|a, b| b.1.cmp(&a.1));
        artists.truncate(5);
        artists
    }

    /// Limpia toda la base de datos de tracks
    fn clear_database(&mut self) {
        self.tracks.clear();
        self.genres.clear();
        self.artists.clear();
    }

    /// Verifica si un archivo es de audio soportado
    fn is_audio_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|s| s.to_str())
            .map(is_audio_file_ext)
            .unwrap_or(false)
    }

    /// Obtiene estadísticas de la biblioteca
    pub fn get_stats(&self) -> PlaylistStats {
        let total_duration: std::time::Duration =
            self.tracks.iter().filter_map(|t| t.duration).sum();

        // Cuenta álbumes únicos
        let total_albums = self
            .tracks
            .iter()
            .filter_map(|t| t.album.as_ref())
            .collect::<std::collections::HashSet<_>>()
            .len();

        PlaylistStats {
            total_tracks: self.tracks.len(),
            total_genres: self.genres.len(),
            total_albums,
            total_duration,
        }
    }

    /// Agrupa pistas por década según su año
    pub fn get_tracks_by_decade(&self) -> HashMap<String, u64> {
        let mut decades = HashMap::new();
        for track in &self.tracks {
            if let Some(year) = track.year {
                let decade = (year / 10) * 10;
                *decades.entry(format!("{}s", decade)).or_insert(0) += 1;
            }
        }
        decades
    }
}

/// Estadísticas de la biblioteca musical
#[derive(Debug)]
pub struct PlaylistStats {
    /// Total de pistas
    pub total_tracks: usize,
    /// Total de géneros únicos
    pub total_genres: usize,
    /// Total de álbumes únicos
    pub total_albums: usize,
    /// Duración total de todas las pistas
    pub total_duration: std::time::Duration,
}

impl PlaylistStats {
    /// Formatea la duración total en formato legible
    pub fn format_duration(&self) -> String {
        let total_seconds = self.total_duration.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else {
            format!("{}m {}s", minutes, seconds)
        }
    }
}
