use anyhow::Result;
use rodio::{OutputStream, Sink, Source};
use std::{path::Path, time::Duration};
use symphonia::core::audio::{AudioBuffer, Signal};

pub struct AudioPlayer {
    press_sound: Option<AudioSource>,
    release_sound: Option<AudioSource>,
    stream: OutputStream,
}

#[derive(Debug, Clone)]
pub struct AudioSource {
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
    pos: usize,
}

impl AudioSource {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        use std::fs::File;

        let file = File::open(path)?;

        // Use symphonia to decode the audio file
        let media_source =
            symphonia::core::io::MediaSourceStream::new(Box::new(file), Default::default());
        let probe_result = symphonia::default::get_probe().format(
            &Default::default(),
            media_source,
            &Default::default(),
            &Default::default(),
        )?;

        let mut format_reader = probe_result.format;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow::anyhow!("No audio track found"))?;

        let track_id = track.id;
        let mut decoder =
            symphonia::default::get_codecs().make(&track.codec_params, &Default::default())?;

        let mut samples = Vec::new();
        let mut channels = 0;
        let mut sample_rate = 44100;

        loop {
            let packet = match format_reader.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break
                }
                Err(err) => return Err(anyhow::anyhow!("Error reading packet: {}", err)),
            };

            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(audio_buf) => {
                    let spec = *audio_buf.spec();
                    channels = spec.channels.count() as u16;
                    sample_rate = spec.rate;

                    match audio_buf {
                        symphonia::core::audio::AudioBufferRef::S16(buf) => {
                            let frames = buf.frames();
                            let ch_count = spec.channels.count();
                            for frame in 0..frames {
                                for ch in 0..ch_count {
                                    let sample = buf.chan(ch)[frame];
                                    samples.push(sample as f32 / i16::MAX as f32);
                                }
                            }
                        }
                        symphonia::core::audio::AudioBufferRef::F32(buf) => {
                            let frames = buf.frames();
                            let ch_count = spec.channels.count();
                            for frame in 0..frames {
                                for ch in 0..ch_count {
                                    samples.push(buf.chan(ch)[frame]);
                                }
                            }
                        }
                        _ => {
                            let mut f32_buf =
                                AudioBuffer::<f32>::new(audio_buf.capacity() as u64, spec);
                            audio_buf.convert(&mut f32_buf);
                            let frames = f32_buf.frames();
                            let ch_count = spec.channels.count();
                            for frame in 0..frames {
                                for ch in 0..ch_count {
                                    samples.push(f32_buf.chan(ch)[frame]);
                                }
                            }
                        }
                    }
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => {
                    // Decode error, skip this packet
                    continue;
                }
                Err(err) => return Err(anyhow::anyhow!("Error decoding packet: {}", err)),
            }
        }

        Ok(AudioSource {
            samples,
            channels,
            sample_rate,
            pos: 0,
        })
    }
}

impl Iterator for AudioSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.samples.get(self.pos).copied()?;
        self.pos += 1;
        Some(sample)
    }
}

impl Source for AudioSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl AudioPlayer {
    pub fn new<P: AsRef<Path>>(press_path: P, release_path: P) -> Result<Self> {
        let mut stream = rodio::OutputStreamBuilder::open_default_stream()
            .map_err(|e| anyhow::anyhow!("Failed to initialize default audio output: {}", e))?;
        stream.log_on_drop(false);

        println!(
            "[Audio] Loading press sound from: {}",
            press_path.as_ref().display()
        );
        let press_sound = match AudioSource::from_file(&press_path) {
            Ok(sound) => {
                println!(
                    "[Audio] Press sound loaded successfully: {} samples, {} channels, {} Hz",
                    sound.samples.len(),
                    sound.channels,
                    sound.sample_rate
                );
                Some(sound)
            }
            Err(e) => {
                println!("[Audio] Failed to load press sound: {}", e);
                None
            }
        };

        println!(
            "[Audio] Loading release sound from: {}",
            release_path.as_ref().display()
        );
        let release_sound = match AudioSource::from_file(&release_path) {
            Ok(sound) => {
                println!(
                    "[Audio] Release sound loaded successfully: {} samples, {} channels, {} Hz",
                    sound.samples.len(),
                    sound.channels,
                    sound.sample_rate
                );
                Some(sound)
            }
            Err(e) => {
                println!("[Audio] Failed to load release sound: {}", e);
                None
            }
        };

        Ok(AudioPlayer {
            press_sound,
            release_sound,
            stream,
        })
    }

    pub fn play_press(&self) {
        // println!("[Audio] play_press() called");
        if let Some(ref sound) = self.press_sound {
            // println!("[Audio] Playing press sound with {} samples", sound.samples.len());
            let sound_clone = sound.clone();
            let sink = Sink::connect_new(self.stream.mixer());
            // println!("[Audio] Audio sink created, playing sound...");
            sink.append(sound_clone);
            sink.detach();
        } else {
            println!("[Audio] No press sound loaded");
        }
    }

    pub fn play_release(&self) {
        // println!("[Audio] play_release() called");
        if let Some(ref sound) = self.release_sound {
            // println!("[Audio] Playing release sound with {} samples", sound.samples.len());
            let sound_clone = sound.clone();
            let sink = Sink::connect_new(self.stream.mixer());
            // println!("[Audio] Audio sink created, playing sound...");
            sink.append(sound_clone);
            sink.detach();
        } else {
            println!("[Audio] No release sound loaded");
        }
    }
}
