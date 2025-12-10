use crate::signal::Signal;
use fon::chan::Channel;
use fon::chan::{Ch16, Ch32};

const TAU: f32 = 6.283_185_5;
pub const SAMPLE_RATE: f32 = 44_100.0;
pub const SAMPLE_PERIOD: f32 = 1.0 / SAMPLE_RATE;

#[derive(Copy, Clone)]
pub struct Const {
    value: f32,
}

impl Const {
    pub fn new(value: f32) -> Self {
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

pub struct Sine {
    freq: Box<dyn Signal>,
    state: f32,
}

impl Sine {
    pub fn new(freq: Box<dyn Signal>) -> Self {
        Self { freq, state: 0.0 }
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

pub struct Gain {
    signal: Box<dyn Signal>,
    gain: f32,
}

impl Gain {
    pub fn new(signal: Box<dyn Signal>, gain: f32) -> Self {
        Self { signal, gain }
    }
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

pub struct Sum {
    a: Box<dyn Signal>,
    b: Box<dyn Signal>,
}

impl Sum {
    pub fn new(a: Box<dyn Signal>, b: Box<dyn Signal>) -> Self {
        Self { a, b }
    }
}

impl Signal for Sum {
    fn sample(&mut self, t: f32) -> Ch32 {
        self.a.sample(t) + self.b.sample(t)
    }

    fn clone_box(&self) -> Box<dyn Signal> {
        Box::new(Sum::new(self.a.clone_box(), self.b.clone_box()))
    }
}

enum AdsrState {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

pub struct Adsr {
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
    pub fn new(gate: Box<dyn Signal>, input: Box<dyn Signal>) -> Self {
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

pub struct Sample {
    gate: Box<dyn Signal>,
    samples: Vec<Ch32>,
    index: usize,
}

impl Sample {
    pub fn new(file: &str, gate: Box<dyn Signal>) -> Self {
        let mut reader = hound::WavReader::open(file).unwrap();
        Sample {
            gate,
            samples: reader
                .samples::<i16>()
                .map(|x| Ch32::from(Ch16::new(x.unwrap())))
                .collect::<Vec<_>>(),
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
        } else if self.index < self.samples.len() {
            let sample = self.samples[self.index];
            self.index += 1;
            sample
        } else {
            Ch32::from(0.0)
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
pub struct Every {
    period: f32,
    duration: f32,
}

impl Every {
    pub fn new(period: f32, duration: f32) -> Self {
        Self { period, duration }
    }
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

pub struct StepSignal {
    steps: Vec<(Box<dyn Signal>, f32)>,
    total_time: f32,
}

impl StepSignal {
    pub fn new(steps: Vec<(Box<dyn Signal>, f32)>) -> Self {
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
        Ch32::from(0.0)
    }

    fn clone_box(&self) -> Box<dyn Signal> {
        let cloned_steps = self
            .steps
            .iter()
            .map(|(sig, x)| (sig.clone_box(), *x))
            .collect::<Vec<_>>();
        Box::new(StepSignal::new(cloned_steps))
    }
}
