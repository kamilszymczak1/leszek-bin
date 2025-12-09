use fon::chan::{Ch16, Ch32, Ch64};
use fon::chan::Channel;
use fon::{Audio, Frame};
use twang::noise::White;
use twang::Synth;

use std::ops::Add;
use std::time::Duration;

/// First ten harmonic volumes of a piano sample (sounds like electric piano).
const HARMONICS: [f32; 10] = [
    0.700, 0.243, 0.229, 0.095, 0.139, 0.087, 0.288, 0.199, 0.124, 0.090,
];

const TAU : f32 = 6.283185307179586;
const SAMPLE_PERIOD : f32 = 1.0 / 48_000.0;

const HALF_STEP: f32 = 1.0594630943592953;

fn note(base: f32, count: f32) -> f32 {
    base * HALF_STEP.powf(count)
}


// State of the synthesizer.
// #[derive(Default)]
// struct Processors {
//     // White noise generator.
//     white: White,
//     // 10 harmonics for 3 pitches.
//     piano: [[Sine; 10]; PITCHES_LEN],
//     adsr: ADSR,
// }

fn save_to_wav(file: &str, audio: Audio<Ch32, 2>) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: audio.sample_rate().get(),
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float
    };

    let mut writer = hound::WavWriter::create(file, spec).unwrap();
    for frame in audio.iter() {
        for chan in frame.channels() {
            writer.write_sample(chan.to_f32()).unwrap();
        }
    }
}

struct ADSR {
    attack: u32,
    delay: u32,
    sustain_level: f32,
    sustain: u32,
    release: u32,
    t: u32,
}

// fn lerp(a: f32, b: f32, t0: f32, t1: f32, t: f32) -> f32 {
//     (t - t0) / (t1 - t0) * (b - a) + a
// }

// impl Default for ADSR {
//     fn default() -> Self {
//         Self::new()
//     }
// }

// impl ADSR {
//     fn new() -> Self {
//         ADSR {
//             attack: 2000,
//             delay: 5000,
//             sustain_level: 0.7,
//             sustain: 8000,
//             release: 10000,
//             t: 0
//         }
//     }

//     fn peek(&self, input: Ch32) -> Ch32 {
//         if self.t < self.attack {
//             Ch32::from(input.to_f32() * lerp(0.0, 1.0, 0.0, self.attack as f32, self.t as f32))
//         } else if self.t < self.attack  + self.delay {
//             let t0 = self.attack as f32;
//             let t1 = (self.attack + self.delay) as f32;
//             Ch32::from(input.to_f32() * lerp(1.0, self.sustain_level, t0, t1, self.t as f32))
//         } else if self.t < self.attack + self.delay + self.sustain {
//             Ch32::from(input.to_f32() * self.sustain_level)
//         } else if self.t < self.attack + self.delay + self.sustain + self.release {
//             let t0 = (self.attack + self.delay + self.sustain) as f32;
//             let t1 = (self.attack + self.delay + self.release + self.sustain) as f32;
//             Ch32::from(input.to_f32() * lerp(self.sustain_level, 0.0, t0, t1, self.t as f32))
//         } else {
//             Ch32::from(0.0)
//         }
//     }

//     fn step(&mut self) {
//         self.t += 1;
//         if self.t % 1000 == 0 {
//             println!("{}", self.peek(Ch32::from(1.0)).to_f32())
//         }
//     }
// }

trait Signal {
    fn sample(&mut self, t : f32) -> Ch32;
}
struct Const {
    value : f32,
}

impl Const {
    fn new(value : f32) -> Self {
        Self { value }
    }
}

impl Signal for Const {
    fn sample(&mut self, _t : f32) -> Ch32 {
        Ch32::from(self.value)
    }
}

impl Signal for f32 {
    fn sample(&mut self, _t : f32) -> Ch32 {
        Ch32::from(*self)
    }
}

struct Sine {
    freq : Box<dyn Signal>,
    state : f32,
}

impl Sine {
    fn new(freq : Box<dyn Signal>) -> Self {
        Self {
            freq,
            state : 0.0,
        }
    }
}

impl Sine {
    fn new_from_const(freq : f32) -> Self {
        Self {
            freq: Box::new(Const::new(freq)),
            state : 0.0,
        }
    }
}

impl Signal for Sine {
    fn sample(&mut self, t : f32) -> Ch32 {
        let out = (self.state).cos();
        self.state = (self.state + TAU * SAMPLE_PERIOD * self.freq.sample(t).to_f32()) % TAU;
        out.into()
    }
}

struct Gain {
    signal : Box<dyn Signal>,
    gain : f32,
}

impl Signal for Gain {
    fn sample(&mut self, t : f32) -> Ch32 {
        Ch32::from(self.signal.sample(t).to_f32() * self.gain)
    }
}

struct Sum {
    a : Box<dyn Signal>,
    b : Box<dyn Signal>,
}

impl Sum {
    fn new(a : Box<dyn Signal>, b : Box<dyn Signal>) -> Self {
        Self { a, b }
    }
}

impl Signal for Sum {
    fn sample(&mut self, t : f32) -> Ch32 {
        self.a.sample(t) + self.b.sample(t)
    }
}

fn linear_combination_of_harmonics(base_freq : Const, harmonics : &[f32]) -> Box<dyn Signal> {
    harmonics.iter().enumerate().fold(
        Box::new(Const::new(0.0)) as Box<dyn Signal>,
        |acc, (i, &vol)| {
            let freq = Const::new(base_freq.value * (i as f32 + 1.0));
            let sine = Sine::new(Box::new(freq));
            let gain = Gain {
                signal: Box::new(sine),
                gain: vol,
            };
            Box::new(Sum {
                a: acc,
                b: Box::new(gain),
            })
        },
    )
}

fn chord_signal(base_freqs : &[f32], harmonics : &[f32]) -> Box<dyn Signal> {
    base_freqs.iter().fold(
        Box::new(Const::new(0.0)) as Box<dyn Signal>,
        |acc, &base_freq| {
            let signal = linear_combination_of_harmonics(Const::new(base_freq), harmonics);
            Box::new(Sum {
                a: acc,
                b: signal,
            })
        },
    )
}

fn main() {

    // Chcemy wyprodukować tablicę floatów
    // to będzie audio

    const PITCHES_LEN: usize = 3;
    const BASE_PITCH: f32 = 220.0;
    /// The three pitches in a perfectly tuned A3 minor chord
    let pitches: [f32; PITCHES_LEN] = [note(BASE_PITCH, 0.0), note(BASE_PITCH, 3.0), note(BASE_PITCH, 7.0)];
    let mut signal = chord_signal(&pitches, &HARMONICS);

    let mut audio = Audio::<Ch32, 2>::with_silence(48_000, 48_000 * 1);

    const VOLUME: f32 = 1.0 / 10.0;
    for (i, frame) in audio.iter_mut().enumerate() {
        let t = i as f32 * SAMPLE_PERIOD;
        let mut sample = signal.sample(t);
        sample = sample * VOLUME;
        frame.channels_mut()[0] = sample;
        frame.channels_mut()[1] = sample;
    }

    let frame = audio.get(0).unwrap();

    save_to_wav("audio.wav", audio);

    println!("First frame: {:?}", frame);
}
