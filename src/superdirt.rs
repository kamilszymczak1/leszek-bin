use std::thread;
use std::time::{Duration, SystemTime};
use num::BigRational;
use num_traits::cast::ToPrimitive;

use std::net::UdpSocket;

use rosc::{self, OscBundle, OscMessage, OscPacket, OscType};

use crate::pattern::{Pattern, Event};
use crate::segment::Segment;
use crate::time::Time;

struct ServerContext {
    start_time: SystemTime,
    cps: f64
}

impl ServerContext {
    fn new(cps: f64) -> Self {
        Self { start_time: SystemTime::now(), cps }
    }
}

#[derive(Clone, Debug)]
pub struct ControlMessage {
    fields: Vec<(String, OscType)>
}

impl ControlMessage {
    pub fn new(fields: Vec<(String, OscType)>) -> Self {
        Self { fields }
    }

    pub fn sound(sound: &str) -> ControlMessage {
        ControlMessage {
            fields: vec![(String::from("s"), sound.into())]
        }
    }
}

fn scale_duration(ctx: &ServerContext, duration_cycles: BigRational) -> Duration {
    Duration::from_secs_f64(duration_cycles.to_f64().unwrap() / ctx.cps)
}

fn cycles_to_system_time(ctx: &ServerContext, time: Time) -> SystemTime {
    ctx.start_time + scale_duration(ctx, time.0)
}

const SUPERDIRT_ADDR: &str = "/dirt/play";

fn encode_message(ctx: &ServerContext, message: Event<ControlMessage>) -> Option<OscPacket> {
    // FIXME: check no reserved fields were used
    // FIXME: remove duplicate fields

    let part = message.part;
    let fields = message.value.fields;
    println!("{:?}", part.whole.clone());
    println!("{:?}", part.part.clone());
    if let Some(whole) = part.whole && whole.start >= part.part.start {
        let start = whole.clone().start;
        let time = cycles_to_system_time(ctx, start.clone());
        let delta: f32 = scale_duration(ctx, whole.duration()).as_secs_f32();

        let mut args: Vec<OscType> = fields
            .into_iter()
            .flat_map(|(key, val)| [OscType::String(key), val])
            .collect();
        
        args.push("delta".into());
        args.push(delta.into());
        args.push("cycle".into());
        args.push((start.cycle_index() as i32).into());
        // TODO: add orbit

        let msg = OscMessage {
            addr: String::from(SUPERDIRT_ADDR),
            args
        };

        let packet = rosc::OscPacket::Bundle(OscBundle { 
            timetag: time.try_into().unwrap(),
            content: vec![OscPacket::Message(msg)]
        });

        Some(packet)
    } else {
        None
    }
}

const SEND_BEFORE: Duration = Duration::from_millis(100);

pub fn run_server<I>(patterns: I)
where
    I: Iterator,
    I::Item: Pattern<ControlMessage>,
{
    let cps = 1.5;
    let ctx = ServerContext::new(cps);
    let mut sent_until = Time::new(0, 1);
    let pats: Vec<I::Item> = patterns.collect();
    let mut bytes = Vec::new();

    let socket = UdpSocket::bind("0.0.0.0:6767").unwrap();

    loop {
        let seg = Segment::new(sent_until.clone(), sent_until.clone() + Time::new(1, 1));

        pats
            .iter()
            .flat_map(|pat| pat.query(seg.clone()))
            .filter_map(|msg| {
                let encoded = encode_message(&ctx, msg);
                encoded
            })
            .for_each(|packet| {
                bytes.clear();
                rosc::encoder::encode_into(&packet, &mut bytes).unwrap();
                if bytes.len() > 0 {
                    socket.send_to(&bytes, "0.0.0.0:57120").unwrap();
                }
            });

        sent_until = sent_until.clone() + Time::new(1, 1);
        let sleep_until = cycles_to_system_time(&ctx, sent_until.clone()) - SEND_BEFORE;
        // TODO: instead of unwrapping dont sleep and log?
        let sleep_duration = sleep_until.duration_since(SystemTime::now()).unwrap();
        thread::sleep(sleep_duration);
    }
}
