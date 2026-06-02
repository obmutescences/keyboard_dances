use anyhow::Result;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};
use std::{path::Path, time::Duration};
use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    errors::Error as SymphoniaError,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

pub struct AudioPlayer {
    press_sound: Option<AudioSource>,
    release_sound: Option<AudioSource>,
    stream: MixerDeviceSink,
    volume: f32,
    #[allow(dead_code)]
    sample_rate_multiplier: f32,
}

#[derive(Debug, Clone)]
pub struct AudioSource {
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
    pos: usize,
    sample_rate_multiplier: f32,
}

impl AudioSource {
    pub fn from_file<P: AsRef<Path>>(path: P, sample_rate_multiplier: f32) -> Result<Self> {
        use std::fs::File;

        let path = path.as_ref();
        let file = File::open(path)?;

        let media_source = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
            hint.with_extension(extension);
        }

        let probe_result = symphonia::default::get_probe().probe(
            &hint,
            media_source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )?;

        let mut format_reader = probe_result;

        let track = format_reader
            .default_track(TrackType::Audio)
            .ok_or_else(|| anyhow::anyhow!("No audio track found"))?;

        let track_id = track.id;
        let codec_params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or_else(|| anyhow::anyhow!("No audio codec parameters found"))?;
        let mut decoder = symphonia::default::get_codecs()
            .make_audio_decoder(codec_params, &AudioDecoderOptions::default())?;

        let mut samples = Vec::new();
        let mut channels = 0;
        let mut sample_rate = 44100;

        loop {
            let packet = match format_reader.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => break,
                Err(SymphoniaError::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(err) => return Err(anyhow::anyhow!("Error reading packet: {}", err)),
            };

            if packet.track_id != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(audio_buf) => {
                    let spec = audio_buf.spec();
                    channels = u16::try_from(spec.channels().count()).unwrap_or(u16::MAX);
                    sample_rate = spec.rate();

                    let start = samples.len();
                    samples.resize(start + audio_buf.samples_interleaved(), 0.0);
                    audio_buf.copy_to_slice_interleaved(&mut samples[start..]);
                }
                Err(SymphoniaError::DecodeError(_)) => {
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
            sample_rate_multiplier,
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

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(self.channels.max(1)).expect("channel count is clamped to be non-zero")
    }

    fn sample_rate(&self) -> SampleRate {
        let sample_rate = (self.sample_rate as f32 * self.sample_rate_multiplier).max(1.0) as u32;
        SampleRate::new(sample_rate).expect("sample rate is clamped to be non-zero")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl AudioPlayer {
    pub fn new<P: AsRef<Path>>(
        press_path: P,
        release_path: P,
        volume: f32,
        sample_rate_multiplier: f32,
    ) -> Result<Self> {
        let mut stream = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| anyhow::anyhow!("Failed to initialize default audio output: {}", e))?;
        stream.log_on_drop(false);

        println!(
            "[Audio] Loading press sound from: {}",
            press_path.as_ref().display()
        );
        let press_sound = match AudioSource::from_file(&press_path, sample_rate_multiplier) {
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
        let release_sound = match AudioSource::from_file(&release_path, sample_rate_multiplier) {
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
            volume,
            sample_rate_multiplier,
        })
    }

    pub fn play_press(&self) {
        // println!("[Audio] play_press() called");
        if let Some(ref sound) = self.press_sound {
            // println!("[Audio] Playing press sound with {} samples", sound.samples.len());
            let sound_clone = sound.clone();
            let player = Player::connect_new(self.stream.mixer());
            player.set_volume(self.volume);
            // println!("[Audio] Audio sink created, playing sound...");
            player.append(sound_clone);
            player.detach();
        } else {
            println!("[Audio] No press sound loaded");
        }
    }

    pub fn play_release(&self) {
        // println!("[Audio] play_release() called");
        if let Some(ref sound) = self.release_sound {
            // println!("[Audio] Playing release sound with {} samples", sound.samples.len());
            let sound_clone = sound.clone();
            let player = Player::connect_new(self.stream.mixer());
            let end_volume = self.volume * 0.2;
            player.set_volume(end_volume);
            // println!("[Audio] Audio sink created, playing sound...");
            player.append(sound_clone);
            player.detach();
        } else {
            println!("[Audio] No release sound loaded");
        }
    }
}
