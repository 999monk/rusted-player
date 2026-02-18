use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

/// Comandos que pueden enviarse al reproductor de audio
#[derive(Debug, Clone)]
pub enum PlayerCommand {
    /// Reproduce una canción individual
    PlaySong(PathBuf),
    /// Reproduce un álbum completo (lista de pistas)
    PlayAlbum(Vec<PathBuf>),
    /// Reproduce pistas en orden aleatorio
    PlayShuffle(Vec<PathBuf>),
    /// Alterna entre pausa y reproducción
    TogglePause,
    /// Establece el volumen (0.0 a 2.0)
    SetVolume(f32),
    /// Incrementa el volumen en 0.1
    VolumeUp,
    /// Decrementa el volumen en 0.1
    VolumeDown,
    /// Detiene la reproducción
    Stop,
    /// Salta a la siguiente pista
    SkipNext,
    /// Cierra el reproductor
    Quit,
}

/// Estados que el reproductor puede reportar
#[derive(Debug, Clone, Copy)]
pub enum PlayerStatus {
    /// Volumen actual (0.0 - 2.0)
    Volume(f32),
}

/// Error posibles al inicializar el reproductor
#[derive(Debug)]
pub enum PlayerError {
    /// No se pudo inicializar el dispositivo de audio
    AudioDeviceError(String),
}

impl std::fmt::Display for PlayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerError::AudioDeviceError(msg) => {
                write!(f, "Error al inicializar el dispositivo de audio: {}", msg)
            }
        }
    }
}

impl std::error::Error for PlayerError {}

/// Servicio de reproducción de audio
pub struct PlayerService {
    sender: Sender<PlayerCommand>,
    /// Canal para recibir actualizaciones de estado del reproductor
    pub receiver: Receiver<PlayerStatus>,
}

impl PlayerService {
    /// Crea un nuevo servicio de reproducción
    ///
    /// Inicializa el hilo de reproducción en segundo plano. Si no hay dispositivo
    /// de audio disponible, el hilo termina inmediatamente pero el servicio sigue
    /// funcionando (los comandos se ignorarán silenciosamente).
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (status_tx, status_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = Self::player_loop(cmd_rx, status_tx);
        });

        Self {
            sender: cmd_tx,
            receiver: status_rx,
        }
    }

    /// Envía un comando al reproductor
    ///
    /// Retorna `Err` si el hilo de reproducción ha terminado
    pub fn send(&self, cmd: PlayerCommand) -> Result<(), mpsc::SendError<PlayerCommand>> {
        self.sender.send(cmd)
    }

    fn player_loop(
        rx: Receiver<PlayerCommand>,
        status_tx: Sender<PlayerStatus>,
    ) -> Result<(), PlayerError> {
        let (_stream, handle) = OutputStream::try_default().map_err(|e| {
            PlayerError::AudioDeviceError(format!(
                "No se pudo obtener dispositivo por defecto: {}",
                e
            ))
        })?;

        let mut sink: Option<Sink> = None;
        let mut current_volume = 1.0f32;

        while let Ok(cmd) = rx.recv() {
            match cmd {
                PlayerCommand::PlaySong(path) => {
                    let _ = Self::play_single_song(&handle, &mut sink, &path, current_volume);
                }
                PlayerCommand::PlayAlbum(tracks) => {
                    let _ = Self::play_tracks(&handle, &mut sink, &tracks, current_volume);
                }
                PlayerCommand::PlayShuffle(mut tracks) => {
                    use rand::seq::SliceRandom;
                    tracks.shuffle(&mut rand::rng());
                    let _ = Self::play_tracks(&handle, &mut sink, &tracks, current_volume);
                }
                PlayerCommand::TogglePause => {
                    if let Some(ref s) = sink {
                        if s.is_paused() {
                            s.play();
                        } else {
                            s.pause();
                        }
                    }
                }
                PlayerCommand::SetVolume(volume) => {
                    current_volume = Self::update_volume(&sink, volume.clamp(0.0, 2.0), &status_tx);
                }
                PlayerCommand::VolumeUp => {
                    current_volume =
                        Self::update_volume(&sink, (current_volume + 0.1).min(2.0), &status_tx);
                }
                PlayerCommand::VolumeDown => {
                    current_volume =
                        Self::update_volume(&sink, (current_volume - 0.1).max(0.0), &status_tx);
                }
                PlayerCommand::Stop => {
                    if let Some(s) = sink.take() {
                        s.stop();
                    }
                }
                PlayerCommand::SkipNext => {
                    if let Some(ref s) = sink {
                        s.skip_one();
                    }
                }
                PlayerCommand::Quit => {
                    // Limpieza explícita antes de salir
                    if let Some(s) = sink.take() {
                        s.stop();
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    fn play_single_song(
        handle: &OutputStreamHandle,
        sink: &mut Option<Sink>,
        path: &PathBuf,
        volume: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Detener reproducción anterior antes de iniciar nueva
        if let Some(s) = sink.take() {
            s.stop();
        }

        let file = File::open(path)
            .map_err(|e| format!("No se pudo abrir '{}': {}", path.display(), e))?;

        // Intentar decodificar el archivo
        // Nota: rodio/symphonia decodifica frames de audio MP3. Si hay frames corruptos,
        // fallará aquí. No hay forma de hacerlo más permisivo sin cambiar bibliotecas,
        // pero al menos manejamos el error gracefully.
        let source = match Decoder::new(BufReader::new(file)) {
            Ok(src) => src,
            Err(e) => {
                // Intentar una segunda vez con un buffer más pequeño (a veces ayuda)
                let file2 = File::open(path).map_err(|_| {
                    format!(
                        "Archivo corrupto o formato no soportado: {}",
                        path.display()
                    )
                })?;
                match Decoder::new(BufReader::with_capacity(4096, file2)) {
                    Ok(src) => src,
                    Err(_) => {
                        return Err(format!(
                            "El archivo tiene frames corruptos o encoding inválido: {}",
                            path.display()
                        )
                        .into());
                    }
                }
            }
        };

        let new_sink = Sink::try_new(handle).map_err(|e| format!("Error de audio: {}", e))?;

        new_sink.set_volume(volume);
        new_sink.append(source);
        *sink = Some(new_sink);

        Ok(())
    }

    fn play_tracks(
        handle: &OutputStreamHandle,
        sink: &mut Option<Sink>,
        tracks: &[PathBuf],
        volume: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Detener reproducción anterior antes de iniciar nueva
        if let Some(s) = sink.take() {
            s.stop();
        }

        let new_sink = Sink::try_new(handle)
            .map_err(|e| format!("No se pudo crear el sink de audio: {}", e))?;

        new_sink.set_volume(volume);

        for path in tracks {
            if let Ok(file) = File::open(path) {
                // Intentar decodificar con buffer estándar
                if let Ok(source) = Decoder::new(BufReader::new(file)) {
                    new_sink.append(source);
                } else if let Ok(file2) = File::open(path) {
                    // Fallback: intentar con buffer pequeño
                    if let Ok(source) = Decoder::new(BufReader::with_capacity(4096, file2)) {
                        new_sink.append(source);
                    }
                }
            }
            // Los archivos que fallan se omiten silenciosamente
        }

        *sink = Some(new_sink);
        Ok(())
    }

    fn update_volume(sink: &Option<Sink>, volume: f32, status_tx: &Sender<PlayerStatus>) -> f32 {
        if let Some(s) = sink {
            s.set_volume(volume);
        }
        if status_tx.send(PlayerStatus::Volume(volume)).is_err() {
            // El receptor se ha desconectado, continuamos de todos modos
        }
        volume
    }
}
