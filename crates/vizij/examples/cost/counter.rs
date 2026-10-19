//! Instructions retired by the calling thread: what a section run on
//! this thread costs, and nothing another thread (a GPU driver's, say)
//! does meanwhile.

/// The calling thread's instruction counter, `None` where none is read
/// (an OS other than macOS and Linux, or Linux without perf events).
pub fn instructions() -> Option<u64> {
    imp::instructions()
}

#[cfg(target_os = "macos")]
mod imp {
    extern "C" {
        /// The kernel's per-thread counters; kind 1 is
        /// `[instructions, cycles]`.
        fn thread_selfcounts(kind: i32, buffer: *mut u64, size: usize) -> i32;
    }

    pub fn instructions() -> Option<u64> {
        let mut counts = [0u64; 2];
        // SAFETY: `counts` is the 16 bytes the call is told it may write.
        let status = unsafe { thread_selfcounts(1, counts.as_mut_ptr(), 16) };
        (status == 0).then_some(counts[0])
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::cell::Cell;

    /// `perf_event_attr` up to its first published size.
    #[repr(C)]
    struct PerfEventAttr {
        kind: u32,
        size: u32,
        config: u64,
        sample_period: u64,
        sample_type: u64,
        read_format: u64,
        flags: u64,
        wakeup_events: u32,
        bp_type: u32,
        config1: u64,
    }
    const PERF_TYPE_HARDWARE: u32 = 0;
    const PERF_COUNT_HW_INSTRUCTIONS: u64 = 1;
    const EXCLUDE_KERNEL: u64 = 1 << 5;
    const EXCLUDE_HV: u64 = 1 << 6;

    thread_local! {
        /// This thread's counter, opened on first use.
        static COUNTER: Cell<Option<i32>> = const { Cell::new(None) };
    }

    fn open() -> Option<i32> {
        let attr = PerfEventAttr {
            kind: PERF_TYPE_HARDWARE,
            size: std::mem::size_of::<PerfEventAttr>() as u32,
            config: PERF_COUNT_HW_INSTRUCTIONS,
            sample_period: 0,
            sample_type: 0,
            read_format: 0,
            flags: EXCLUDE_KERNEL | EXCLUDE_HV,
            wakeup_events: 0,
            bp_type: 0,
            config1: 0,
        };
        // SAFETY: `attr` outlives the call; pid 0 and cpu -1 count this
        // thread on any CPU.
        let fd = unsafe { libc::syscall(libc::SYS_perf_event_open, &attr, 0, -1, -1, 0) };
        (fd >= 0).then_some(fd as i32)
    }

    pub fn instructions() -> Option<u64> {
        let fd = COUNTER.with(|counter| {
            if counter.get().is_none() {
                counter.set(open());
            }
            counter.get()
        })?;
        let mut count = 0u64;
        // SAFETY: reads the 8 bytes of `count`.
        let read = unsafe { libc::read(fd, (&mut count as *mut u64).cast(), 8) };
        (read == 8).then_some(count)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod imp {
    pub fn instructions() -> Option<u64> {
        None
    }
}
