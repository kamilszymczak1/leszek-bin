use fon::Audio;
use fon::chan::{Ch16, Ch32};
use fon::chan::Channel;

/// First ten harmonic volumes of a piano sample (sounds like electric piano).
const HARMONICS: [f32; 10] = [
    0.700, 0.243, 0.229, 0.095, 0.139, 0.087, 0.288, 0.199, 0.124, 0.090,
];

const TAU: f32 = 6.283_185_5;
const SAMPLE_RATE: f32 = 44_100.0;
const SAMPLE_PERIOD: f32 = 1.0 / SAMPLE_RATE;

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
fn lerp(a: f32, b: f32, t0: f32, t1: f32, t: f32) -> f32 {
    (t - t0) / (t1 - t0) * (b - a) + a
}

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

trait Signal {
    fn sample(&mut self, t: f32) -> Ch32;

    fn clone_box(&self) -> Box<dyn Signal>;
}

#[derive(Copy, Clone)]
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

    fn clone_box(&self) -> Box<dyn Signal> {
        Box::new(*self)
    }
}

impl Signal for f32 {
    fn sample(&mut self, _t: f32) -> Ch32 {
        Ch32::from(*self)
    }

    fn clone_box(&self) -> Box<dyn Signal> {
        Box::new(*self)
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

    fn clone_box(&self) -> Box<dyn Signal> {
        Box::new(Sine::new(self.freq.clone_box()))
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

    fn clone_box(&self) -> Box<dyn Signal> {
        Box::new(Gain {
            signal: self.signal.clone_box(),
            gain: self.gain,
        })
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

   fn clone_box(&self) -> Box<dyn Signal> {
       Box::new(Sum::new(
           self.a.clone_box(),
           self.b.clone_box(),
       ))
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
    steps: Vec<(Box<dyn Signal>, f32)>,
    total_time: f32,
}

impl StepSignal {
    fn new(steps: Vec<(Box<dyn Signal>, f32)>) -> Self {
        let total_time = steps.iter().map(|(_, dur)| *dur).sum();
        Self { steps, total_time }
    }
}

impl Signal for StepSignal {
    fn sample(&mut self, t: f32) -> Ch32 {
        let t = t % self.total_time;
        let mut accumulated_time = 0.0;
        for (signal, duration) in &mut self.steps {
            if t < accumulated_time + *duration {
                return signal.sample(t - accumulated_time);
            }
            accumulated_time += *duration;
        }
        return Ch32::from(0.0);
    }


    fn clone_box(&self) -> Box<dyn Signal> {
        let cloned_steps = self.steps.iter().map(|(sig, x)| {
            (sig.clone_box(), *x)
        }).collect::<Vec<_>>();
        Box::new(StepSignal::new(cloned_steps))
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

    let freq_signal = Box::new(StepSignal::new(
        freqs
            .into_iter()
            .map(|(f, d)| (play_note(Const::new(f), &HARMONICS), d))
            .collect(),
    ));
    let gate_signal = Box::new(StepSignal::new(
        gates
            .into_iter()
            .map(|(g, d)| (Box::new(Const::new(g)) as Box<dyn Signal>, d))
            .collect(),
    ));

    (freq_signal, gate_signal)
}

enum AdsrState {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

struct Adsr {
    state: AdsrState,
    gate: Box<dyn Signal>,
    input: Box<dyn Signal>,
    peak_level: f32,
    attack: f32,
    decay: f32,
    sustain_level: f32,
    release: f32,
    beg_value: f32,
    value: f32,
    gate_last: bool,
}

impl Adsr {
    fn new(gate: Box<dyn Signal>, input: Box<dyn Signal>) -> Self {
        Adsr {
            state: AdsrState::Idle,
            gate,
            input,
            peak_level: 1.0,
            attack: 0.01,
            decay: 0.3,
            sustain_level: 0.5,
            release: 0.01,
            beg_value: 0.0,
            value: 0.0,
            gate_last: false,
        }
    }
}

impl Signal for Adsr {
    fn sample(&mut self, t: f32) -> Ch32 {
        let gate = self.gate.sample(t).to_f32() > 0.0;
        match (self.gate_last, gate) {
            (false, true) => {
                self.beg_value = self.value;
                self.state = AdsrState::Attack;
            }
            (true, false) => {
                self.beg_value = self.value;
                self.state = AdsrState::Release;
            }
            _ => {}
        }
        self.gate_last = gate;

        match self.state {
            AdsrState::Idle => {
                self.beg_value = 0.0;
            }

            AdsrState::Attack => {
                let step = self.peak_level / (self.attack * SAMPLE_RATE).max(1.0);
                self.value += step;
                if self.value >= self.peak_level {
                    self.value = self.peak_level;
                    self.state = AdsrState::Decay;
                }
            }

            AdsrState::Decay => {
                let step =
                    (self.peak_level - self.sustain_level) / (self.decay * SAMPLE_RATE).max(1.0);
                self.value -= step;
                if self.value <= self.sustain_level {
                    self.value = self.sustain_level;
                    self.state = AdsrState::Sustain;
                }
            }

            AdsrState::Sustain => self.value = self.sustain_level,

            AdsrState::Release => {
                let step = self.beg_value / (self.release * SAMPLE_RATE).max(1.0);
                self.value -= step;
                if self.value <= 0.0 {
                    self.value = 0.0;
                    self.state = AdsrState::Idle;
                }
            }
        }

        let input = self.input.sample(t);
        Ch32::from(self.value * input.to_f32())
    }

    fn clone_box(&self) -> Box<dyn Signal> {
        Box::new(Adsr::new(self.gate.clone_box(), self.input.clone_box()))
    }
}

struct Sample {
    gate: Box<dyn Signal>,
    samples: Vec<Ch32>,
    index: usize,
}

impl Sample {
    fn new(file: &str, gate: Box<dyn Signal>) -> Self {
        let mut reader = hound::WavReader::open(file).unwrap();
        Sample {
            gate,
            samples: reader.samples::<i16>().map(|x| Ch32::from(Ch16::new(x.unwrap()))).collect::<Vec<_>>(),
            index: 0,
        }
    }
}

impl Signal for Sample {
    fn sample(&mut self, t: f32) -> Ch32 {
        let gate = self.gate.sample(t).to_f32();
        if gate <= 0.0 {
            self.index = 0;
            Ch32::from(0.0)
        } else {
            if self.index < self.samples.len() {
                let sample = self.samples[self.index];
                self.index += 1;
                sample
            } else {
                Ch32::from(0.0)
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Signal> {
        Box::new(Sample {
            gate: self.gate.clone_box(),
            samples: self.samples.clone(),
            index: 0,
        })
    }
}

#[derive(Clone, Copy)]
struct Every {
    period: f32,
    duration: f32,
}

impl Signal for Every {
    fn sample(&mut self, t: f32) -> Ch32 {
        Ch32::new(if (t % self.period) < self.duration {
            1.0
        } else {
            0.0
        })
    }

    fn clone_box(&self) -> Box<dyn Signal> {
        Box::new(*self)
    }
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
    let B0 = note(C, -1.0);

    let notes = Vec::from([
        (E, 1.5),
        (E, 0.5),
        (G, 0.5 * 1.5),
        (E, 0.5 * 1.5),
        (D, 0.5),
        (C, 2.0),
        (B0, 2.0),
    ]);


    let (freq_signal, gate_signal) = generate_melody(&notes, 120);

    let kick = Gain { 
        signal: Box::new(Sample::new("samples/kick.wav", Box::new(Every { period: 0.5, duration: 0.3 }))),
        gain: 1.0,
    };
    let mut signal_adsr = Sum::new(Box::new(Adsr::new(gate_signal, freq_signal)), kick.clone_box());

    let mut audio = Audio::<Ch32, 2>::with_silence(SAMPLE_RATE as u32, (SAMPLE_RATE as usize) * 5);

    const VOLUME: f32 = 4.0 / 10.0;
    for (i, frame) in audio.iter_mut().enumerate() {
        let t = i as f32 * SAMPLE_PERIOD;
        let mut sample = signal_adsr.sample(t);
        sample = sample * VOLUME;
        *frame = frame.pan(sample, 0.0);
    }

    let frame = audio.get(0).unwrap();

    save_to_wav("audio.wav", audio);

    println!("First frame: {:?}", frame);
}
