use fon::chan::Ch16;
use fon::chan::Channel;
use fon::{Audio, Frame};
use twang::noise::White;
use twang::ops::Gain;
use twang::osc::Sine;
use twang::Synth;
use std::fs::File;
use std::io::BufWriter;

use rodio::{OutputStream, Source};
use std::time::Duration;


/// First ten harmonic volumes of a piano sample (sounds like electric piano).
const HARMONICS: [f32; 10] = [
    0.700, 0.243, 0.229, 0.095, 0.139, 0.087, 0.288, 0.199, 0.124, 0.090,
];
/// The three pitches in a perfectly tuned A3 minor chord
const PITCHES: [f32; 3] = [220.0, 220.0 * 32.0 / 27.0, 220.0 * 3.0 / 2.0];
/// Volume of the piano
const VOLUME: f32 = 3.0 / 3.0;

// State of the synthesizer.
#[derive(Default)]
struct Processors {
    // White noise generator.
    white: White,
    // 10 harmonics for 3 pitches.
    piano: [[Sine; 10]; 3],
}



struct FonSource {
    data: Audio<Ch16, 2>,
    pos: usize,
    last_channel: usize,
}

impl Iterator for FonSource {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.data.get(self.pos)?;
        let value = frame.channels()[self.last_channel];
        self.last_channel = (self.last_channel + 1) % 2;
        if self.last_channel == 0 {
            self.pos += 1;
        }
        Some(value.to_f32())
    }
}

impl rodio::Source for FonSource {
    fn current_span_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { 48_000 }
    fn total_duration(&self) -> Option<Duration> { None }
}


fn main() {
    // Initialize audio
    let mut audio = Audio::<Ch16, 2>::with_silence(48_000, 48_000 * 5);
    // Create audio processors
    let mut proc = Processors::default();
    // Adjust phases of harmonics.
    for pitch in proc.piano.iter_mut() {
        for harmonic in pitch.iter_mut() {
            harmonic.shift(proc.white.step());
        }
    }
    // Build synthesis algorithm
    let mut synth = Synth::new(proc, |proc, mut frame: Frame<_, 2>| {
        for (s, pitch) in proc.piano.iter_mut().zip(PITCHES.iter()) {
            for ((i, o), v) in s.iter_mut().enumerate().zip(HARMONICS.iter()) {
                // Get next sample from oscillator.
                let sample = o.step(pitch * (i + 1) as f32);
                // Pan the generated harmonic center
                frame = frame.pan(Gain.step(sample, (v * VOLUME).into()), 0.0);
            }
        }
        frame
    });
    // Synthesize 5 seconds of audio
    println!("Rendering...");
    synth.stream(audio.sink());

    let frame = audio.get(0).unwrap();

    println!("First frame: {:?}", frame);

    let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();

    let fon_source = FonSource {
        data: audio,
        pos: 0,
        last_channel: 0,
    };

    stream_handle.mixer().add(fon_source);

    std::thread::sleep(std::time::Duration::from_secs(5));
}