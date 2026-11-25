/// Returns the current process resident memory (Working Set on Windows, RSS on Linux) in megabytes.
#[cfg(windows)]
pub fn current_ram_mb() -> f64 {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let mut counters: PROCESS_MEMORY_COUNTERS = zeroed();
        counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0 {
            counters.WorkingSetSize as f64 / (1024.0 * 1024.0)
        } else {
            0.0
        }
    }
}

#[cfg(target_os = "linux")]
pub fn current_ram_mb() -> f64 {
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        if let Some(rss_pages_str) = statm.split_whitespace().nth(1) {
            if let Ok(rss_pages) = rss_pages_str.parse::<u64>() {
                // Standard Linux page size is 4096 bytes.
                return (rss_pages * 4096) as f64 / (1024.0 * 1024.0);
            }
        }
    }
    0.0
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn current_ram_mb() -> f64 {
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_reading_is_positive() {
        let mb = current_ram_mb();
        assert!(mb > 0.0, "Expected positive RAM reading, got {mb}");
    }
}
