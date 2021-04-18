use core::usize;

#[derive(Debug, Clone, Copy)]
pub enum InterruptEvent {
    Tick,
    Encoder(i8),
}

const QUEUE_LENGTH: usize = 32;

// pub type EventQueue = ([Option<InterruptEvent>; QUEUE_LENGTH], usize);

#[derive(Debug)]
pub struct EventQueue {
    queue: [Option<InterruptEvent>; QUEUE_LENGTH],
    head: usize,
    tail: usize,
}

impl EventQueue {
    pub const fn new() -> Self {
        EventQueue {
            queue: [None; QUEUE_LENGTH],
            head: 0,
            tail: 0,
        }
    }

    pub fn take(&mut self) -> Option<InterruptEvent> {
        let evt = self.queue[self.head].take()?;

        self.head += 1;
        if self.head >= self.queue.len() {
            self.head = 0;
        }
        Some(evt)
    }

    pub fn put(&mut self, evt: InterruptEvent) {
        self.queue[self.tail] = Some(evt);
        self.tail += 1;
        if self.tail >= self.queue.len() {
            self.tail = 0;
        }
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}
