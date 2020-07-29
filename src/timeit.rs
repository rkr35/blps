use std::fmt::Display;
use std::time::Instant;

use log::info;

pub struct TimeIt<T: Display> {
    begin: Instant,
    task: T,
}

impl<T: Display> TimeIt<T> {
    pub fn new(task: T) -> Self {
        Self {
            begin: Instant::now(),
            task,
        }
    }
}

impl<T: Display> Drop for TimeIt<T> {
    fn drop(&mut self) {
        let elapsed = self.begin.elapsed();
        let ms = elapsed.as_millis();
        info!("It took {} ms to {}.", ms, self.task);
    }
}
