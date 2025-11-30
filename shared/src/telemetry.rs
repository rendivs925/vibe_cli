use std::time::Instant;

pub struct Telemetry {
    start: Instant,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}
