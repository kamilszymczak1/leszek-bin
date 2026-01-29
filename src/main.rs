mod note;
mod scale;
mod signal;
mod signals;
mod utils;
mod scale;
mod note;

mod superdirt;
mod pattern;
mod lang;
mod parser;

use crate::pattern::{Pattern, slowcat, cycled, fastcat};
use crate::superdirt::{ControlMessage, run_server};

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
