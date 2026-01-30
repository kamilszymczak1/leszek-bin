mod note;
mod scale;
mod segment;
mod time;

mod lang;
mod parser;
mod pattern;
mod superdirt;

use crate::pattern::{Pattern, cycled, fastcat, slowcat};
use crate::superdirt::{ControlMessage, run_server};

fn main() {
    fn s(name: &'static str) -> pattern::BoxPattern<ControlMessage> {
        pattern::cycled(ControlMessage::sound(name)).boxed()
    }

    println!(
        "{}",
        pattern::display_pattern(
            &fastcat(
                [
                    cycled(0).boxed(),
                    slowcat([cycled(1), cycled(2)].into_iter()).boxed()
                ]
                .into_iter()
            )
            .boxed()
        )
    );
    println!(
        "{}",
        pattern::display_pattern(
            &lang::eval_pattern(parser::parse("cat([1, 2])").unwrap()).unwrap()
        )
    );
    let _pat1 =
        fastcat([s("bd"), slowcat([s("sn"), s("hh")].into_iter()).boxed()].into_iter()).boxed();
    let _pat2 = pattern::cycled(ControlMessage::sound("arpy")).boxed();

    let code = std::fs::read_to_string("demo.code").unwrap();
    let parsed = parser::parse(&code).unwrap();
    let pat3 = lang::eval_control_pattern(parsed).unwrap();
    // println!("{}", pattern::display_pattern(&pat3));
    run_server(
        vec![
            // pat1,
            // pat2,
            pat3,
        ]
        .into_iter(),
    );
}
