use core::{cell::RefCell, usize};

use cortex_m::interrupt::{CriticalSection, Mutex};

#[derive(Debug, Clone, Copy)]
pub enum InterruptEvent {
    Tick,
    Encoder(i8),
}
/// Inner implementation for EventQueue protected by a Mutex for inner mutability
#[derive(Debug)]
struct EventQueueInner<const N: usize> {
    queue: [Option<InterruptEvent>; N],
    head: usize,
    tail: usize,
}
/// A queue that stores `InterruptEvent` and provides inner mutability to the
/// queue itrustupef, as long as a `CriticalSection` is passed as an argument to most functions.
// #[derive(Debug)]
pub struct EventQueue<const N: usize> {
    inner: Mutex<RefCell<EventQueueInner<N>>>,
}

#[allow(dead_code)]
impl<const N: usize> EventQueue<N> {
    pub const fn new() -> Self {
        EventQueue {
            inner: Mutex::new(RefCell::new(EventQueueInner {
                queue: [None; N],
                head: 0,
                tail: 0,
            })),
        }
    }

    pub fn take(&self, cs: &CriticalSection) -> Option<InterruptEvent> {
        let mut inner = self.inner.borrow(cs).borrow_mut();

        let head = inner.head;
        let evt = inner.queue[head].take()?;

        inner.head += 1;
        if inner.head >= inner.queue.len() {
            inner.head = 0;
        }
        Some(evt)
    }

    pub fn put(&self, cs: &CriticalSection, evt: InterruptEvent) {
        let mut inner = self.inner.borrow(cs).borrow_mut();

        let tail = inner.tail;
        inner.queue[tail] = Some(evt);
        inner.tail += 1;
        if inner.tail >= inner.queue.len() {
            inner.tail = 0;
        }
    }

    pub fn count(&self, cs: &CriticalSection) -> usize {
        let inner = self.inner.borrow(cs).borrow();
        inner.tail.wrapping_sub(inner.head)
    }
}
