use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

pub struct RingBuffer {
    samples: Box<[AtomicU32]>,
    write_pos: AtomicUsize,
    mask: usize,
}

impl RingBuffer {
    pub fn new() -> Self {
        let samples = Vec::new().into_boxed_slice();
        Self {
            samples,
            write_pos: AtomicUsize::new(0),
            mask: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        assert!(
            capacity.is_power_of_two(),
            "Capacity must be a power of two."
        );
        assert!(
            capacity != 0,
            "If you want to create a ringbuffer with 0 capacity, use `new()` instead."
        );
        let samples = (0..capacity)
            .map(|_| AtomicU32::new(0.0f32.to_bits()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            samples,
            //start from capacity to make a waveform start rendering from the right of the screen.
            write_pos: AtomicUsize::new(capacity),
            mask: capacity - 1,
        }
    }

    pub fn push(&self, sample: f32) {
        let pos = self.write_pos.load(Ordering::Relaxed);
        self.samples[pos & self.mask].store(sample.to_bits(), Ordering::Relaxed);
        self.write_pos.store(pos.wrapping_add(1), Ordering::Release);
    }

    pub fn copy_latest(&self, output: &mut [f32]) {
        let end = self.write_pos.load(Ordering::Acquire);
        let count = output.len().min(self.samples.len()).min(end);

        let start = end - count;

        for (i, v) in output.iter_mut().enumerate().take(count) {
            let index = (start + i) & self.mask;
            let bits = self.samples[index].load(Ordering::Relaxed);
            *v = f32::from_bits(bits);
        }
    }
}
