//! Deterministic event queue for the discrete-event simulation.
//!
//! Events pop in time order. Events due at the same instant pop in the order
//! they were pushed, so a run never depends on heap internals.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use types::Timestamp;

/// One queued event. Ordering looks only at `(at, seq)`, so `E` needs no traits.
struct Entry<E> {
    at: Timestamp,
    seq: u64,
    event: E,
}

impl<E> PartialEq for Entry<E> {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.seq) == (other.at, other.seq)
    }
}

impl<E> Eq for Entry<E> {}

impl<E> PartialOrd for Entry<E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<E> Ord for Entry<E> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.at, self.seq).cmp(&(other.at, other.seq))
    }
}

/// A min-queue of events keyed by time, FIFO among equal times.
pub struct Scheduler<E> {
    heap: BinaryHeap<Reverse<Entry<E>>>,
    next_seq: u64,
}

impl<E> Scheduler<E> {
    /// An empty queue.
    pub fn new() -> Self {
        Scheduler {
            heap: BinaryHeap::new(),
            next_seq: 0,
        }
    }

    /// Queue `event` to fire at `at`.
    pub fn push(&mut self, at: Timestamp, event: E) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.heap.push(Reverse(Entry { at, seq, event }));
    }

    /// Remove and return the earliest event; ties come out in push order.
    pub fn pop(&mut self) -> Option<(Timestamp, E)> {
        self.heap.pop().map(|Reverse(e)| (e.at, e.event))
    }

    /// The time of the earliest event, without removing it.
    pub fn peek_time(&self) -> Option<Timestamp> {
        self.heap.peek().map(|Reverse(e)| e.at)
    }

    /// Number of queued events.
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// True when no events are queued.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

impl<E> Default for Scheduler<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pops_in_time_order() {
        let mut s = Scheduler::new();
        s.push(Timestamp(30), "c");
        s.push(Timestamp(10), "a");
        s.push(Timestamp(20), "b");
        assert_eq!(s.pop(), Some((Timestamp(10), "a")));
        assert_eq!(s.pop(), Some((Timestamp(20), "b")));
        assert_eq!(s.pop(), Some((Timestamp(30), "c")));
        assert_eq!(s.pop(), None);
    }

    #[test]
    fn ties_break_by_insertion() {
        let mut s = Scheduler::new();
        s.push(Timestamp(5), "a");
        s.push(Timestamp(5), "b");
        s.push(Timestamp(5), "c");
        assert_eq!(s.pop(), Some((Timestamp(5), "a")));
        assert_eq!(s.pop(), Some((Timestamp(5), "b")));
        assert_eq!(s.pop(), Some((Timestamp(5), "c")));
    }

    #[test]
    fn empty_returns_none() {
        let mut s: Scheduler<u8> = Scheduler::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.peek_time(), None);
        assert_eq!(s.pop(), None);
    }
}
