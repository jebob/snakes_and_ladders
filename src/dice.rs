use rand::rngs::ThreadRng;
use rand::Rng;

pub const DIE_SIZE: usize = 6; // Must be >= 1

pub trait Roll {
    // Either a random die or a mock.
    fn roll(&mut self) -> usize;
}

impl Roll for ThreadRng {
    fn roll(&mut self) -> usize {
        self.gen_range(1, DIE_SIZE + 1)
    }
}

pub struct Unrollable {} // Fallback class, used for testing only

impl Roll for Unrollable {
    fn roll(&mut self) -> usize {
        panic!("Can't roll this!")
    }
}

pub struct MockDie {
    // gives some predetermined results, then panics. Used for testing only
    pub queued_results: Vec<usize>, // Popped RIGHT to LEFT!!
}

impl Roll for MockDie {
    fn roll(&mut self) -> usize {
        self.queued_results.pop().unwrap()
    }
}
