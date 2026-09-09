//! Packet id window of a single connection.
//!
//! MQTT requires packet identifiers to be unique among the in-flight messages
//! of one connection. A slot index maps directly to `id = index + 1`, so the
//! window never hands out a duplicate and `--max-inflight` doubles as the
//! maximum number of un-acknowledged messages.

/// Fixed-size packet id window.
pub struct Slots {
    used: Vec<bool>,
    cursor: usize,
}

impl Slots {
    /// A window with `count` slots (at least one).
    pub fn new(count: usize) -> Self {
        Slots {
            used: vec![false; count.max(1)],
            cursor: 0,
        }
    }

    /// Number of slots.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.used.len()
    }

    /// Claim the next free packet id, `None` when the window is full.
    pub fn take(&mut self) -> Option<u16> {
        let n = self.capacity();
        for i in 0..n {
            let idx = (self.cursor + i) % n;
            if !self.used[idx] {
                self.used[idx] = true;
                self.cursor = (idx + 1) % n;
                return Some(idx as u16 + 1);
            }
        }
        None
    }

    /// Return an id to the window. Ids of 0 or out of range are ignored.
    pub fn release(&mut self, pid: u16) {
        if pid == 0 {
            return;
        }
        let idx = pid as usize - 1;
        if idx < self.capacity() {
            self.used[idx] = false;
        }
    }

    /// Number of claimed slots.
    pub fn in_use(&self) -> usize {
        self.used.iter().filter(|u| **u).count()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ids_are_unique_until_full() {
        let mut s = Slots::new(3);
        let ids: Vec<u16> = (0..3).filter_map(|_| s.take()).collect();
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 3);
        assert_eq!(ids, vec![1, 2, 3]);
        assert_eq!(s.take(), None);
        assert_eq!(s.in_use(), 3);
    }

    #[test]
    fn released_ids_are_reused() {
        let mut s = Slots::new(2);
        let a = s.take().unwrap();
        let b = s.take().unwrap();
        s.release(b);
        assert_eq!(s.in_use(), 1);
        assert_eq!(s.take(), Some(b));
        s.release(a);
        s.release(b);
        assert_eq!(s.in_use(), 0);
    }

    #[test]
    fn ignores_out_of_range_releases() {
        let mut s = Slots::new(4);
        s.release(0);
        s.release(9999);
        s.release(5);
        assert_eq!(s.in_use(), 0);
        assert!(s.take().is_some());
    }

    #[test]
    fn zero_sized_window_degrades_to_one_slot() {
        let mut s = Slots::new(0);
        assert_eq!(s.capacity(), 1);
        assert_eq!(s.take(), Some(1));
        assert_eq!(s.take(), None);
    }

    #[test]
    fn window_of_max_size_wraps_without_duplicates() {
        let mut s = Slots::new(64);
        let mut live = HashSet::new();
        for _ in 0..100_000 {
            if let Some(pid) = s.take() {
                assert!(live.insert(pid), "duplicate id {pid} while in flight");
                if live.len() == 40 {
                    let old = live.iter().next().copied().unwrap();
                    live.remove(&old);
                    s.release(old);
                }
            } else {
                assert_eq!(live.len(), s.capacity());
                let old = live.iter().next().copied().unwrap();
                live.remove(&old);
                s.release(old);
            }
        }
    }
}
