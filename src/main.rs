mod notes;
mod signal;
mod signals;
mod utils;

mod superdirt;
mod pattern;
mod lang;
mod parser;

use crate::pattern::{Pattern, slowcat, cycled, fastcat};
use crate::superdirt::{ControlMessage, run_server};

use signal::Signal;
use signals::{Const, Gain, Sine, StepSignal, Sum};

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
    fn s(name: &'static str) -> pattern::BoxPattern<ControlMessage> {
        pattern::cycled(ControlMessage::sound(name)).boxed()
    }

    println!("{}", pattern::display_pattern(&fastcat([cycled(0).boxed(), slowcat([cycled(1), cycled(2)].into_iter()).boxed()].into_iter()).boxed()));
    println!("{}", pattern::display_pattern(&lang::eval_pattern(parser::parse("cat([1, 2])").unwrap()).unwrap()));
    let pat1 = fastcat([s("bd"), slowcat([s("sn"), s("hh")].into_iter()).boxed()].into_iter()).boxed();
    let pat2 = pattern::cycled(ControlMessage::sound("arpy")).boxed();

    let code = r#"s(cat(["bd", cat(["sn", "hh"])]))"#;
    let parsed = parser::parse(code).unwrap();
    let pat3 = lang::eval_control_pattern(parsed).unwrap();
    run_server(vec![
        pat1,
        pat2,
        pat3,
    ].into_iter());
}
