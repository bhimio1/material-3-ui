//! Number formatting for labels: sizes, transfer rates, durations,
//! frequencies and percentages.

/// "1.4 GiB", "512 KiB", "0 B" (powers of 1024, IEC units).
pub fn bytes(n: u64) -> String {
    scaled(n, 1024.0, &["B", "KiB", "MiB", "GiB", "TiB", "PiB"])
}

/// "1.5 KB", "5.0 MB": a file size the way file managers label it
/// (powers of 1024, short units).
pub fn file_size(n: u64) -> String {
    scaled(n, 1024.0, &["B", "KB", "MB", "GB", "TB", "PB"])
}

fn scaled(n: u64, base: f64, units: &[&str]) -> String {
    let mut v = n as f64;
    let mut unit = 0;
    while v >= base && unit + 1 < units.len() {
        v /= base;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else if v >= 100.0 {
        format!("{v:.0} {}", units[unit])
    } else {
        format!("{v:.1} {}", units[unit])
    }
}

/// "3.2 MB/s" (decimal, like download speeds).
pub fn rate(bytes_per_sec: f64) -> String {
    const UNITS: [&str; 5] = ["B/s", "kB/s", "MB/s", "GB/s", "TB/s"];
    let mut v = if bytes_per_sec.is_finite() { bytes_per_sec.max(0.0) } else { 0.0 };
    let mut unit = 0;
    while v >= 1000.0 && unit + 1 < UNITS.len() {
        v /= 1000.0;
        unit += 1;
    }
    if unit == 0 || v >= 100.0 {
        format!("{v:.0} {}", UNITS[unit])
    } else {
        format!("{v:.1} {}", UNITS[unit])
    }
}

/// "2 d 4 h", "3 h 12 min", "5 min", "45 s".
pub fn duration(secs: f64) -> String {
    let s = if secs.is_finite() { secs.max(0.0) as u64 } else { 0 };
    let (d, h, m) = (s / 86400, (s / 3600) % 24, (s / 60) % 60);
    if d > 0 {
        format!("{d} d {h} h")
    } else if h > 0 {
        format!("{h} h {m} min")
    } else if m > 0 {
        format!("{m} min")
    } else {
        format!("{s} s")
    }
}

/// "2.40 GHz" or "800 MHz", from MHz.
pub fn mhz(v: f32) -> String {
    if v >= 1000.0 {
        format!("{:.2} GHz", v / 1000.0)
    } else {
        format!("{v:.0} MHz")
    }
}

/// "26%" from 0.256, clamped to 0–100.
pub fn percent(fraction: f32) -> String {
    let f = if fraction.is_finite() { fraction } else { 0.0 };
    format!("{:.0}%", (f * 100.0).clamp(0.0, 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1536), "1.5 KiB");
        assert_eq!(bytes(16 * 1024 * 1024 * 1024), "16.0 GiB");
        assert_eq!(bytes(500 * 1024 * 1024), "500 MiB");
        assert_eq!(bytes(u64::MAX), "16384 PiB");
        assert_eq!(file_size(512), "512 B");
        assert_eq!(file_size(1536), "1.5 KB");
        assert_eq!(file_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn rates_durations_and_frequencies() {
        assert_eq!(rate(999.0), "999 B/s");
        assert_eq!(rate(2_500_000.0), "2.5 MB/s");
        assert_eq!(rate(-3.0), "0 B/s");
        assert_eq!(rate(f64::NAN), "0 B/s");
        assert_eq!(duration(45.0), "45 s");
        assert_eq!(duration(300.0), "5 min");
        assert_eq!(duration(3.0 * 3600.0 + 720.0), "3 h 12 min");
        assert_eq!(duration(2.0 * 86400.0 + 4.0 * 3600.0), "2 d 4 h");
        assert_eq!(mhz(3800.0), "3.80 GHz");
        assert_eq!(mhz(800.0), "800 MHz");
        assert_eq!(percent(0.256), "26%");
        assert_eq!(percent(1.7), "100%");
        assert_eq!(percent(f32::NAN), "0%");
    }
}
