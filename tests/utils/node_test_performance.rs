use std::time::{Duration, Instant};

pub enum Category {
    Fast,
    Medium,
    Slow,
}

pub struct PerformanceCheck {
    start: Instant,
    category: Category,
}

impl PerformanceCheck {
    pub fn new(category: Category) -> Self {
        Self {
            start: Instant::now(),
            category,
        }
    }

    pub fn check(&self) -> bool {
        let elapsed = self.start.elapsed();
        let check_result = match self.category {
            Category::Fast => elapsed <= Duration::from_millis(15),
            Category::Medium => elapsed <= Duration::from_millis(50),
            Category::Slow => elapsed <= Duration::from_millis(200),
        };

        if !check_result {
            println!("Check failed. Elapsed time: {:?}", elapsed);
        }

        check_result
    }
}