use rodio::{Decoder, OutputStream, Sink};
use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

pub enum PlayerCommand {
    PlaySong(PathBuf),
    PlayAlbum(Vec<PathBuf>),
    PlayShuffle(Vec<PathBuf>),
    TogglePause,
    SetVolume(f32),
    VolumeUp,
    VolumeDown,
    Stop,
    SkipNext,
    Quit,
}

pub enum PlayerStatus {
    Volume(f32),
}

pub struct PlayerService {
    sender: Sender<PlayerCommand>,
    pub receiver: Receiver<PlayerStatus>,
}

impl PlayerService {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (status_tx, status_rx) = mpsc::channel();

        thread::spawn(move || Self::player_loop(cmd_rx, status_tx));

        Self {
            sender: cmd_tx,
            receiver: status_rx,
        }
    }

    pub fn send(&self, cmd: PlayerCommand) {
        let _ = self.sender.send(cmd);
    }

    fn player_loop(rx: Receiver<PlayerCommand>, status_tx: Sender<PlayerStatus>) {
        let (_stream, handle) = OutputStream::try_default().unwrap();
        let mut sink: Option<Sink> = None;
        let mut current_volume = 1.0f32;

        while let Ok(cmd) = rx.recv() {
            match cmd {
                PlayerCommand::PlaySong(path) => {
                    if let Ok(file) = File::open(&path) {
                        let source = Decoder::new(BufReader::new(file)).unwrap();
                        let new_sink = Sink::try_new(&handle).unwrap();
                        new_sink.set_volume(current_volume);
                        new_sink.append(source);
                        sink = Some(new_sink);
                    }
                }
                PlayerCommand::PlayAlbum(tracks) => {
                    let new_sink = Sink::try_new(&handle).unwrap();
                    new_sink.set_volume(current_volume);
                    for path in &tracks {
                        if let Ok(file) = File::open(path) {
                            if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                new_sink.append(source);
                            }
                        }
                    }
                    sink = Some(new_sink);
                }
                PlayerCommand::PlayShuffle(mut tracks) => {
                    use rand::seq::SliceRandom;
                    tracks.shuffle(&mut rand::rng());
                    let new_sink = Sink::try_new(&handle).unwrap();
                    new_sink.set_volume(current_volume);
                    for path in &tracks {
                        if let Ok(file) = File::open(path) {
                            if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                new_sink.append(source);
                            }
                        }
                    }
                    sink = Some(new_sink);
                }
                PlayerCommand::TogglePause => {
                    if let Some(ref s) = sink {
                        if s.is_paused() {
                            s.play();
                        }else {
                            s.pause();
                        }
                    }

                }
                PlayerCommand::SetVolume(volume) => {
                    current_volume = volume.max(0.0);
                    if let Some(ref s) = sink {
                        s.set_volume(current_volume);
                    }
                    let _ = status_tx.send(PlayerStatus::Volume(current_volume));
                }
                PlayerCommand::VolumeUp => {
                    current_volume = (current_volume + 0.1).min(2.0);
                    if let Some(ref s) = sink {
                        s.set_volume(current_volume);
                    }
                    let _ = status_tx.send(PlayerStatus::Volume(current_volume));
                }
                PlayerCommand::VolumeDown => {
                    current_volume = (current_volume - 0.1).max(0.0);
                    if let Some(ref s) = sink {
                        s.set_volume(current_volume);
                    }
                    let _ = status_tx.send(PlayerStatus::Volume(current_volume));
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
                PlayerCommand::Quit => break,
            }
        }
    }
}
