//! Thread-to-core pinning. Linux only.

use std::io;

/// Pin the calling thread to a specific CPU core.
///
/// Fails if the core number is out of range, or if the process lacks
/// `CAP_SYS_NICE` (rare — pinning within your own allowed set usually
/// needs no privileges).
pub fn pin_to_core(core: usize) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        libc::CPU_SET(core, &mut set);
        let ret = libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
        if ret != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = core;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "pin_to_core is only implemented on Linux",
        ))
    }
}

/// Number of logical CPUs visible to this process.
pub fn available_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}
