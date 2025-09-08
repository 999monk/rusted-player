use std::collections::HashMap;
use std::path::{Path, PathBuf};
use lofty::{Accessor, AudioFile, Probe, TaggedFileExt};
use levenshtein::levenshtein;



#[derive(Debug, Clone)]
pub struct TrackMetadata {
    pub path: PathBuf,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub duration: Option<std::time::Duration>,
}

#[derive(Debug)]
pub struct PlaylistService {
    tracks: Vec<TrackMetadata>,
    genres: HashMap<String, Vec<usize>>,
    artists: HashMap<String, Vec<usize>>,
    albums: HashMap<String, Vec<usize>>,
}

fn normalize_genre(genre: &str) -> String {
    genre.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

impl PlaylistService {
    pub fn new() -> Self {
        PlaylistService {
            tracks: Vec::new(),
            genres: HashMap::new(),
            artists: HashMap::new(),
            albums: HashMap::new(),
        }
    }

    /// extrae metadata directorio
    pub fn scan_directory(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.clear_database();

        for entry in walkdir::WalkDir::new(dir_path) {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && self.is_audio_file(path) {
                if let Ok(metadata) = self.extract_metadata(path) {
                    let index = self.tracks.len();

                    // por género
                    if let Some(ref genre) = metadata.genre {
                        let normalized_genre = normalize_genre(genre);
                        let mut similar_genre = None;
                        for existing_genre in self.genres.keys() {
                            if levenshtein(&normalized_genre, &normalize_genre(existing_genre)) <= 2 {
                                similar_genre = Some(existing_genre.clone());
                                break;
                            }
                        }

                        if let Some(similar) = similar_genre {
                            self.genres.get_mut(&similar).unwrap().push(index);
                        } else {
                            self.genres.entry(genre.clone()).or_insert_with(Vec::new).push(index);
                        }
                    }

                    // por album
                    if let Some(ref album) = metadata.album {
                        self.albums.entry(album.clone()).or_insert_with(Vec::new).push(index);
                    }

                    // por artista
                    if let Some(ref artist) = metadata.artist {
                        self.artists.entry(artist.clone()).or_insert_with(Vec::new).push(index);
                    }

                    self.tracks.push(metadata);
                }
            }
        }

        Ok(())
    }

    /// extrae metadata archivo
    fn extract_metadata(&self, path: &Path) -> Result<TrackMetadata, Box<dyn std::error::Error>> {
        let tagged_file = Probe::open(path)?.read()?;

        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
        let properties = tagged_file.properties();

        let metadata = TrackMetadata {
            path: path.to_path_buf(),
            album: tag.as_ref().and_then(|t| t.album().map(|s| s.to_string())),
            artist: tag.as_ref().and_then(|t| t.artist().map(|s| s.to_string())),
            genre: tag.as_ref().and_then(|t| t.genre().map(|s| s.to_string())),
            year: tag.and_then(|t| t.year()),
            duration: Some(properties.duration()),
        };
        Ok(metadata)
    }

    /// generar playlist por género
    pub fn get_playlist_by_genre(&self, genre: &str) -> Vec<PathBuf> {
        self.genres
            .get(genre)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&i| self.tracks[i].path.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// obtiene géneros 
    pub fn get_genres(&self) -> Vec<String> {
        let mut genres: Vec<_> = self.genres.keys().cloned().collect();
        genres.sort();
        genres
    }

    pub fn get_top_artists(&self) -> Vec<(String, u64)> {
        let mut artists: Vec<(String, u64)> = self.artists.iter().map(|(artist, tracks)| (artist.clone(), tracks.len() as u64)).collect();
        artists.sort_by(|a, b| b.1.cmp(&a.1));
        artists.truncate(5);
        artists
    }

    /// obtener info track
    #[allow(dead_code)]
    pub fn get_track_info(&self, path: &Path) -> Option<&TrackMetadata> {
        self.tracks.iter().find(|track| track.path == path)
    }

    /// limpiar
    fn clear_database(&mut self) {
        self.tracks.clear();
        self.genres.clear();
        self.artists.clear();
        self.albums.clear();
    }

    /// verificar archivos
    fn is_audio_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            matches!(ext.to_lowercase().as_str(), "mp3" | "flac" | "ogg" | "wav" | "m4a" | "aac" | "wma")
        } else {
            false
        }
    }

    /// stats
    pub fn get_stats(&self) -> PlaylistStats {
        let total_duration: std::time::Duration = self.tracks
            .iter()
            .filter_map(|t| t.duration)
            .sum();

        PlaylistStats {
            total_tracks: self.tracks.len(),
            total_genres: self.genres.len(),
            total_albums: self.albums.len(),
            total_duration,
        }
    }

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

#[derive(Debug)]
pub struct PlaylistStats {
    pub total_tracks: usize,
    pub total_genres: usize,
    pub total_albums: usize,
    pub total_duration: std::time::Duration,
}

impl PlaylistStats {
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