use fon::chan::Ch32;

pub trait Signal {
    fn sample(&mut self, t: f32) -> Ch32;

    fn clone_box(&self) -> Box<dyn Signal>;
}
