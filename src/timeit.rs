use std::time::Instant;

use log::info;

pub struct TimeIt {
    begin: Instant,
    task: String,
}

impl TimeIt {
    pub fn new(task: &str) -> Self {
        let task = String::from(task);
        let begin = Instant::now();
        Self { begin, task, }
    }
}

impl Drop for TimeIt {
    fn drop(&mut self) {
        let elapsed = self.begin.elapsed();
        let ms = elapsed.as_millis();
        info!("It took {} ms to {}.", ms, self.task);
    }
}