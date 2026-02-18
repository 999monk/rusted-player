# AGENTS.md - rusted-player

Guide for AI agents working on this Rust music player TUI project.

## Build & Run Commands

```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run the application
cargo run

# Run with music directory argument
cargo run -- "/path/to/music"
```

## Lint & Format Commands

```bash
# Check code without building
cargo check

# Run linter (clippy)
cargo clippy

# Run linter with all features
cargo clippy --all-features

# Format code
cargo fmt

# Check formatting without changes
cargo fmt -- --check
```

## Testing

```bash
# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Run tests in a specific module
cargo test module_name
```

## Code Style Guidelines

### Imports Order
1. Standard library (`std::`)
2. External crates (e.g., `ratatui::`, `crossterm::`, `serde::`)
3. Internal modules (`crate::models::`, `crate::services::`)

```rust
use std::{fs, io};
use ratatui::{layout::Layout, widgets::Block};
use crate::models::config::Config;
```

### Naming Conventions
- **Types/Structs/Enums**: PascalCase (`PlayerService`, `TrackMetadata`, `PlayerCommand`)
- **Functions/Variables**: snake_case (`load_config`, `music_path`)
- **Modules**: snake_case (`config_service`, `player_service`)
- **Constants**: SCREAMING_SNAKE_CASE (if any)

### Error Handling
- Use `Result<T, E>` for recoverable errors
- Use `?` operator for propagation
- Use `unwrap()` only for truly unrecoverable cases (e.g., thread spawn)
- Return `io::Result<()>` for main operations

```rust
pub fn save_config(config: &Config) -> Result<(), std::io::Error> {
    let config_str = serde_json::to_string_pretty(config)?;
    fs::write(config_path, config_str)?;
    Ok(())
}
```

### Types & Structs
- Use `pub` for fields that need external access
- Derive common traits: `#[derive(Debug, Clone)]`
- Use `Option<T>` for nullable fields
- Use `PathBuf` for file paths, not `String`

```rust
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    pub path: PathBuf,
    pub album: Option<String>,
    pub year: Option<u32>,
}
```

### Comments
- Use Spanish for comments (project convention)
- Document public functions with `///`
- Keep comments concise and descriptive

```rust
/// extrae metadata directorio
pub fn scan_directory(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>>
```

### Enums
- Use enums for command/status patterns
- Derive `Clone` if needed for channels

```rust
pub enum PlayerCommand {
    PlaySong(PathBuf),
    TogglePause,
    SetVolume(f32),
}
```

### Module Organization
- Use `mod.rs` files to expose child modules
- Mark modules as `pub` for external visibility

```rust
// services/mod.rs
pub mod config_service;
pub mod player_service;
```

### Async & Threading
- Use `std::sync::mpsc` for channels
- Spawn threads for blocking I/O (audio playback)
- Use `thread::spawn` with closures

### Dependencies
Key crates in use:
- `ratatui` - Terminal UI
- `crossterm` - Terminal control
- `rodio` - Audio playback
- `lofty` - Metadata extraction
- `serde` - Serialization
- `walkdir` - Directory traversal
- `rand` - Randomization

## Project Structure

```
src/
├── main.rs           # Entry point, CLI args
├── models/           # Data structures
│   ├── mod.rs
│   └── config.rs
└── services/         # Business logic
    ├── mod.rs
    ├── config_service.rs
    ├── metadata_service.rs
    ├── player_service.rs
    ├── playlist_storage_service.rs
    └── ui_manager.rs
```

## Important Notes

- **Rust Edition**: 2024
- Config stored as JSON next to executable (`config.json`)
- Supports audio formats: mp3, flac, ogg, wav, m4a, aac, wma
- Uses rodio for audio, lofty for metadata
- No existing tests - add tests in `#[cfg(test)]` modules
