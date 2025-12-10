use lazy_static::lazy_static;

const HALF_STEP: f32 = 1.0594631;

fn note(base: f32, steps: f32) -> f32 {
    base * HALF_STEP.powf(steps)
}

lazy_static! {
    pub static ref C4: f32 = 261.63;
    pub static ref B3: f32 = note(*C4, -1.0);
    pub static ref D4: f32 = note(*C4, 2.0);
    pub static ref E4: f32 = note(*C4, 4.0);
    pub static ref F4: f32 = note(*C4, 5.0);
    pub static ref G4: f32 = note(*C4, 7.0);
    pub static ref A4: f32 = note(*C4, 9.0);
    pub static ref B4: f32 = note(*C4, 11.0);
}
