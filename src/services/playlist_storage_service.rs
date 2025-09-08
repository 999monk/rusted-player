use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Playlist {
    pub name: String,
    pub tracks: Vec<PathBuf>,
}

pub fn save_playlist(playlist: &Playlist) -> Result<(), std::io::Error> {
    let playlists_dir = PathBuf::from(".playlists");
    if !playlists_dir.exists() {
        std::fs::create_dir(&playlists_dir)?;
    }

    let playlist_path = playlists_dir.join(format!("{}.json", playlist.name));
    let playlist_json = serde_json::to_string_pretty(playlist)?;
    std::fs::write(playlist_path, playlist_json)?;

    Ok(())
}

pub fn load_playlists() -> Result<Vec<Playlist>, std::io::Error> {
    let playlists_dir = PathBuf::from(".playlists");
    if !playlists_dir.exists() {
        return Ok(vec![]);
    }

    let mut playlists = vec![];
    for entry in std::fs::read_dir(playlists_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let playlist_json = std::fs::read_to_string(path)?;
            let playlist: Playlist = serde_json::from_str(&playlist_json)?;
            playlists.push(playlist);
        }
    }

    Ok(playlists)
}

pub fn delete_playlist(playlist_name: &str) -> Result<(), std::io::Error> {
    let playlists_dir = PathBuf::from(".playlists");
    let playlist_path = playlists_dir.join(format!("{}.json", playlist_name));

    if playlist_path.exists() {
        std::fs::remove_file(playlist_path)?;
    }

    Ok(())
}
