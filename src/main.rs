mod notes;
mod signal;
mod signals;
mod utils;

use fon::Audio;
use fon::chan::Ch32;
use notes::{B3, C4, D4, E4, G4};
use signal::Signal;
use signals::{Adsr, Const, Every, Gain, Sample, Sine, StepSignal, Sum};
use signals::{SAMPLE_PERIOD, SAMPLE_RATE};
use utils::save_to_wav;

/// First ten harmonic volumes of a piano sample (sounds like electric piano).
const HARMONICS: [f32; 10] = [
    0.700, 0.243, 0.229, 0.095, 0.139, 0.087, 0.288, 0.199, 0.124, 0.090,
];

fn play_note(base_freq: f32, harmonics: &[f32]) -> Box<dyn Signal> {
    harmonics.iter().enumerate().fold(
        Box::new(Const::new(0.0)) as Box<dyn Signal>,
        |acc, (i, &vol)| {
            let freq = Const::new(base_freq * (i as f32 + 1.0));
            let sine = Sine::new(Box::new(freq));
            let gain = Gain::new(Box::new(sine), vol);
            Box::new(Sum::new(acc, Box::new(gain)))
        },
    )
}

// fn chord_signal(base_freqs: &[f32], harmonics: &[f32]) -> Box<dyn Signal> {
//     base_freqs.iter().fold(
//         Box::new(Const::new(0.0)) as Box<dyn Signal>,
//         |acc, &base_freq| {
//             let signal = play_note(base_freq, harmonics);
//             Box::new(Sum::new(acc, signal))
//         },
//     )
// }

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
            .map(|(f, d)| (play_note(f, &HARMONICS), d))
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

fn main() {
    let notes = Vec::from([
        (*E4, 1.5),
        (*E4, 0.5),
        (*G4, 0.5 * 1.5),
        (*E4, 0.5 * 1.5),
        (*D4, 0.5),
        (*C4, 2.0),
        (*B3, 2.0),
    ]);

    let (freq_signal, gate_signal) = generate_melody(&notes, 120);

    let kick = Gain::new(
        Box::new(Sample::new(
            "samples/kick.wav",
            Box::new(Every::new(0.5, 0.3)),
        )),
        3.0,
    );
    let mut signal_adsr = Sum::new(
        Box::new(Adsr::new(gate_signal, freq_signal)),
        kick.clone_box(),
    );

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
