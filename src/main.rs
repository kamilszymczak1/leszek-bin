use fon::Audio;
use fon::chan::Ch32;
use fon::chan::Channel;

/// First ten harmonic volumes of a piano sample (sounds like electric piano).
const HARMONICS: [f32; 10] = [
    0.700, 0.243, 0.229, 0.095, 0.139, 0.087, 0.288, 0.199, 0.124, 0.090,
];

const TAU: f32 = 6.283_185_5;
const SAMPLE_PERIOD: f32 = 1.0 / 48_000.0;

fn note(base: f32, count: f32) -> f32 {
    base * HALF_STEP.powf(count)
}

const HALF_STEP: f32 = 1.059_463_1;

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
        sample_format: hound::SampleFormat::Float,
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
    fn sample(&mut self, t: f32) -> Ch32;
}
struct Const {
    value: f32,
}

impl Const {
    fn new(value: f32) -> Self {
        Self { value }
    }
}

impl Signal for Const {
    fn sample(&mut self, _t: f32) -> Ch32 {
        Ch32::from(self.value)
    }
}

impl Signal for f32 {
    fn sample(&mut self, _t: f32) -> Ch32 {
        Ch32::from(*self)
    }
}

struct Sine {
    freq: Box<dyn Signal>,
    state: f32,
}

impl Sine {
    fn new(freq: Box<dyn Signal>) -> Self {
        Self { freq, state: 0.0 }
    }
}

impl Sine {
    fn new_from_const(freq: f32) -> Self {
        Self {
            freq: Box::new(Const::new(freq)),
            state: 0.0,
        }
    }
}

impl Signal for Sine {
    fn sample(&mut self, t: f32) -> Ch32 {
        let out = (self.state).cos();
        self.state = (self.state + TAU * SAMPLE_PERIOD * self.freq.sample(t).to_f32()) % TAU;
        out.into()
    }
}

struct Gain {
    signal: Box<dyn Signal>,
    gain: f32,
}

impl Signal for Gain {
    fn sample(&mut self, t: f32) -> Ch32 {
        Ch32::from(self.signal.sample(t).to_f32() * self.gain)
    }
}

struct Sum {
    a: Box<dyn Signal>,
    b: Box<dyn Signal>,
}

impl Sum {
    fn new(a: Box<dyn Signal>, b: Box<dyn Signal>) -> Self {
        Self { a, b }
    }
}

impl Signal for Sum {
    fn sample(&mut self, t: f32) -> Ch32 {
        self.a.sample(t) + self.b.sample(t)
    }
}

fn play_note(base_freq: Const, harmonics: &[f32]) -> Box<dyn Signal> {
    harmonics.iter().enumerate().fold(
        Box::new(Const::new(0.0)) as Box<dyn Signal>,
        |acc, (i, &vol)| {
            let freq = Const::new(base_freq.value * (i as f32 + 1.0));
            let sine = Sine::new(Box::new(freq));
            let gain = Gain {
                signal: Box::new(sine),
                gain: vol,
            };
            Box::new(Sum::new(acc, Box::new(gain)))
        },
    )
}

fn chord_signal(base_freqs: &[f32], harmonics: &[f32]) -> Box<dyn Signal> {
    base_freqs.iter().fold(
        Box::new(Const::new(0.0)) as Box<dyn Signal>,
        |acc, &base_freq| {
            let signal = play_note(Const::new(base_freq), harmonics);
            Box::new(Sum::new(acc, signal))
        },
    )
}

struct StepSignal {
    steps: Vec<(f32, f32)>,
    total_time: f32,
}

impl StepSignal {
    fn new(steps: Vec<(f32, f32)>) -> Self {
        let total_time = steps.iter().map(|(_, dur)| *dur).sum();
        Self { steps, total_time }
    }
}

impl Signal for StepSignal {
    fn sample(&mut self, t: f32) -> Ch32 {
        let t = t % self.total_time;
        let mut accumulated_time = 0.0;
        for (freq, duration) in &self.steps {
            accumulated_time += *duration;
            if t < accumulated_time {
                return Ch32::from(*freq);
            }
        }
        return Ch32::from(0.0);
    }
}

fn generate_melody(notes: &[(f32, f32)], bpm: u32) -> (Box<dyn Signal>, Box<dyn Signal>) {
    let mut freqs = Vec::new();
    let mut gates = Vec::new();

    let multiplier = 60.0 / bpm as f32;

    let silence_period = 0.02;

    for &(freq, dur) in notes {
        freqs.push((freq, dur * multiplier));
        gates.push((1.0, (dur - silence_period) * multiplier));
        gates.push((0.0, silence_period * multiplier));
    }

    (
        Box::new(StepSignal::new(freqs)),
        Box::new(StepSignal::new(gates)),
    )
}

fn main() {
    const PITCHES_LEN: usize = 3;
    const BASE_PITCH: f32 = 130.8;

    let C: f32 = 261.63;
    let D: f32 = note(C, 2.0);
    let E: f32 = note(C, 4.0);
    let F: f32 = note(C, 5.0);
    let G: f32 = note(C, 7.0);
    let A: f32 = note(C, 9.0);

    let notes = Vec::from([
        (E, 0.25 * 1.5),
        (E, 0.125 * 1.5),
        (G, 0.125 * 1.5),
        (E, 0.125 * 1.5),
        (D, 0.125),
    ]);

    let (freq_signal, gate_signal) = generate_melody(&notes, 120);

    let mut audio = Audio::<Ch32, 2>::with_silence(48_000, 48_000);

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
