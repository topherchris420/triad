//! Fixed-capacity circular history buffer with an O(1) running sum.
//!
//! Capacity is fixed at construction (startup); `push` never allocates. The running sum is
//! re-derived exactly from the buffer every time the write index wraps to 0, which bounds
//! floating-point drift from repeated add/subtract to one window's worth of operations.

/// Fixed-capacity circular buffer of `f64` with an O(1) running sum.
#[derive(Clone, Debug)]
pub struct RingBuffer {
    buf: Box<[f64]>,
    head: usize, // next write position
    len: usize,
    sum: f64,
    wraps: u64,
}

impl RingBuffer {
    /// Buffer holding at most `capacity` values.
    ///
    /// # Panics
    /// If `capacity == 0`.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be > 0");
        RingBuffer {
            buf: vec![0.0; capacity].into_boxed_slice(),
            head: 0,
            len: 0,
            sum: 0.0,
            wraps: 0,
        }
    }

    /// Maximum number of stored values.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }
    /// Number of stored values.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    /// Whether nothing has been pushed yet.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Whether the buffer holds `capacity` values (further pushes evict the oldest).
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len == self.buf.len()
    }
    /// Number of times the write index has wrapped back to slot 0.
    pub fn wraps(&self) -> u64 {
        self.wraps
    }

    /// Append a value, evicting the oldest when full. Never allocates.
    #[inline]
    pub fn push(&mut self, x: f64) {
        let cap = self.buf.len();
        if self.len == cap {
            self.sum -= self.buf[self.head];
        } else {
            self.len += 1;
        }
        self.buf[self.head] = x;
        self.sum += x;
        self.head += 1;
        if self.head == cap {
            self.head = 0;
            self.wraps += 1;
            // Exact resync in slot order (deterministic).
            let mut s = 0.0;
            for &v in &self.buf[..self.len] {
                s += v;
            }
            self.sum = s;
        }
    }

    /// Sum of the stored values.
    #[inline]
    pub fn sum(&self) -> f64 {
        self.sum
    }

    /// Mean of the stored values; NaN when empty (handles the not-yet-full case that the
    /// article's simplified example ignores).
    #[inline]
    pub fn mean(&self) -> f64 {
        if self.len == 0 {
            f64::NAN
        } else {
            self.sum / self.len as f64
        }
    }

    /// i-th oldest element.
    pub fn get(&self, i: usize) -> Option<f64> {
        if i >= self.len {
            return None;
        }
        let cap = self.buf.len();
        let oldest = (self.head + cap - self.len) % cap;
        Some(self.buf[(oldest + i) % cap])
    }

    /// Oldest-to-newest copy (allocates; for tests/inspection only).
    pub fn to_vec(&self) -> Vec<f64> {
        (0..self.len).map(|i| self.get(i).unwrap()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_fill_mean_is_over_filled_slots() {
        let mut r = RingBuffer::new(4);
        assert!(r.mean().is_nan());
        r.push(1.0);
        r.push(3.0);
        assert_eq!(r.len(), 2);
        assert_eq!(r.mean(), 2.0);
    }

    #[test]
    fn wraparound_keeps_last_n_in_order() {
        let mut r = RingBuffer::new(3);
        for x in 1..=8 {
            r.push(x as f64);
        }
        assert_eq!(r.to_vec(), vec![6.0, 7.0, 8.0]);
        assert_eq!(r.sum(), 21.0);
        assert_eq!(r.wraps(), 2);
        assert!(r.is_full());
    }

    #[test]
    fn running_sum_tracks_naive_sum_over_many_wraps() {
        let mut r = RingBuffer::new(7);
        let mut hist = Vec::new();
        let mut x: u64 = 0x9E3779B97F4A7C15;
        for _ in 0..100_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let v = (x % 10_000) as f64 / 7.0 - 700.0;
            r.push(v);
            hist.push(v);
            let start = hist.len().saturating_sub(7);
            let naive: f64 = hist[start..].iter().sum();
            assert!(
                (r.sum() - naive).abs() <= 1e-9 * naive.abs().max(1.0),
                "{} vs {naive}",
                r.sum()
            );
        }
    }

    #[test]
    fn capacity_one() {
        let mut r = RingBuffer::new(1);
        r.push(5.0);
        r.push(6.0);
        assert_eq!(r.to_vec(), vec![6.0]);
        assert_eq!(r.mean(), 6.0);
    }
}
