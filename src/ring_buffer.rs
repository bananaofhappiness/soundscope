use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Single-producer/single-consumer ring buffer with "overwrite the oldest
/// samples" semantics.
pub struct RingBuffer {
    /// `UnsafeCell` lets the producer mutate the samples through `&self`
    /// without a lock;
    samples: UnsafeCell<Box<[f32]>>,
    write_pos: AtomicUsize,
    mask: usize,
}

// SAFETY: `RingBuffer` is used strictly as SPSC.
unsafe impl Sync for RingBuffer {}

impl RingBuffer {
    pub fn new() -> Self {
        Self {
            samples: Vec::new().into_boxed_slice().into(),
            write_pos: AtomicUsize::new(0),
            mask: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        debug_assert!(
            capacity.is_power_of_two(),
            "Capacity must be a power of two."
        );
        debug_assert!(
            capacity != 0,
            "If you want to create a ringbuffer with 0 capacity, use `new()` instead."
        );
        Self {
            samples: vec![0.0; capacity].into_boxed_slice().into(),
            // start from capacity to make a waveform start rendering from the right of the screen.
            write_pos: AtomicUsize::new(capacity),
            mask: capacity - 1,
        }
    }

    pub fn push_slice(&self, data: &[f32]) {
        let mut src = data;
        // SAFETY: only the producer calls this.
        let samples = unsafe { &mut *self.samples.get() };
        if samples.is_empty() {
            return;
        }
        while !src.is_empty() {
            let pos = self.write_pos.load(Ordering::Relaxed);
            let slot = pos & self.mask;
            // contiguous free run from `slot` up to the end of the storage
            let n = (samples.len() - slot).min(src.len());
            samples[slot..slot + n].copy_from_slice(&src[..n]);
            self.write_pos.store(pos.wrapping_add(n), Ordering::Release);
            src = &src[n..];
        }
    }

    pub fn copy_latest(&self, output: &mut [f32]) -> usize {
        let pos = self.write_pos.load(Ordering::Acquire);
        // SAFETY: read-only consumer access; see the `Sync` impl.
        let samples = unsafe { &*self.samples.get() };
        let count = output.len().min(samples.len()).min(pos);
        if count == 0 {
            return pos;
        }

        let start = (pos - count) & self.mask;
        let end = pos & self.mask;

        if start < end {
            output[..count].copy_from_slice(&samples[start..start + count]);
        } else {
            // the window wraps around the end of the storage
            let tail = samples.len() - start;
            output[..tail].copy_from_slice(&samples[start..]);
            output[tail..count].copy_from_slice(&samples[..end]);
        }
        pos
    }

    pub fn get_pos(&self) -> usize {
        self.write_pos.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_is_quiet() {
        let rb = RingBuffer::new();
        rb.push_slice(&[1.0, 2.0]);
        let mut out = [0.0; 4];
        let pos = rb.copy_latest(&mut out);
        assert_eq!(pos, 0);
        assert_eq!(out, [0.0; 4]);
    }

    #[test]
    fn fresh_buffer_reads_as_zeros() {
        let rb = RingBuffer::with_capacity(8);
        let mut out = [7.0; 4];
        let pos = rb.copy_latest(&mut out);
        assert_eq!(pos, 8);
        assert_eq!(out, [0.0; 4]);
    }

    #[test]
    fn copy_latest_partial_window() {
        let rb = RingBuffer::with_capacity(8);
        rb.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let mut out = [0.0; 3];
        rb.copy_latest(&mut out);
        assert_eq!(out, [3.0, 4.0, 5.0]);
    }

    #[test]
    fn copy_latest_wrapped() {
        let rb = RingBuffer::with_capacity(8);
        rb.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
        let mut out = [0.0; 8];
        rb.copy_latest(&mut out);
        assert_eq!(out, [3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
    }

    #[test]
    fn chunked_push_slice_matches_single() {
        let block: Vec<f32> = (0..25).map(|i| i as f32).collect();

        // callbacks deliver many small blocks, including ones that straddle
        // the end of the storage
        let chunked = RingBuffer::with_capacity(8);
        for chunk in block.chunks(3) {
            chunked.push_slice(chunk);
        }

        let single = RingBuffer::with_capacity(8);
        single.push_slice(&block);

        let mut a = [0.0; 8];
        let mut b = [0.0; 8];
        chunked.copy_latest(&mut a);
        single.copy_latest(&mut b);
        assert_eq!(a, b);
        assert_eq!(a, [17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0]);
    }
}
