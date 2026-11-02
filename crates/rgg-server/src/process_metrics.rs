// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Optional process observations; no OS reads while holding the telemetry lock.
use rgg_application::telemetry::{ProcessSample, RuntimeTelemetry};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

pub(crate) async fn run(telemetry: Arc<RuntimeTelemetry>, stop: CancellationToken) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = stop.cancelled() => return,
            _ = interval.tick() => {}
        }
        // Linux procfs produces one bounded status record. No directory walks,
        // subprocesses, arbitrary paths, per-request tasks or queued probes.
        telemetry.process_sample(collect());
    }
}

fn collect() -> ProcessSample {
    // Age starts before OS reads, so a delayed probe cannot refresh old data.
    let observed_at = Instant::now();
    #[cfg(target_os = "linux")]
    let (cpu_time, resident_bytes) = (linux::cpu_time(), linux::resident_bytes());
    #[cfg(not(target_os = "linux"))]
    let (cpu_time, resident_bytes) = (None, None);
    ProcessSample {
        observed_at,
        cpu_time,
        resident_bytes,
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{fs::File, io::Read, time::Duration};
    const STATUS_LIMIT: usize = 16 * 1024;

    pub(super) fn cpu_time() -> Option<Duration> {
        let value = rustix::time::clock_gettime_dynamic(rustix::time::DynamicClockId::Known(
            rustix::time::ClockId::ProcessCPUTime,
        ))
        .ok()?;
        let seconds = u64::try_from(value.tv_sec).ok()?;
        let nanos = u32::try_from(value.tv_nsec).ok()?;
        if nanos >= 1_000_000_000 {
            return None;
        }
        Some(Duration::new(seconds, nanos))
    }

    pub(super) fn resident_bytes() -> Option<u64> {
        read_rss(File::open("/proc/self/status").ok()?)
    }

    fn read_rss(source: impl Read) -> Option<u64> {
        let mut bytes = Vec::with_capacity(4096);
        source
            .take((STATUS_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > STATUS_LIMIT {
            return None;
        }
        parse_rss(&bytes)
    }

    fn parse_rss(status: &[u8]) -> Option<u64> {
        let mut result = None;
        for line in status.split(|value| *value == b'\n') {
            let Some(value) = line.strip_prefix(b"VmRSS:") else {
                continue;
            };
            if result.is_some() {
                return None;
            }
            let mut fields = value
                .split(u8::is_ascii_whitespace)
                .filter(|part| !part.is_empty());
            let number = fields.next()?;
            if number.is_empty() || !number.iter().all(u8::is_ascii_digit) {
                return None;
            }
            if fields.next()? != b"kB" || fields.next().is_some() {
                return None;
            }
            result = Some(
                std::str::from_utf8(number)
                    .ok()?
                    .parse::<u64>()
                    .ok()?
                    .checked_mul(1024)?,
            );
        }
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn status_parser_ignores_non_utf8_names_and_checks_units_duplicates_and_overflow() {
            assert_eq!(
                parse_rss(b"Name:\t\xff\nVmSize: 9000 kB\nVmRSS:\t1234 kB\nThreads: 2\n"),
                Some(1234 * 1024)
            );
            assert_eq!(parse_rss(b"VmRSS: 0 kB\n"), Some(0));
            for invalid in [
                &b"VmSize: 100 kB\n"[..],
                &b"VmRSS: 10 MB\n"[..],
                &b"VmRSS: 10 kB extra\n"[..],
                &b"VmRSS: -1 kB\n"[..],
                &b"VmRSS: +1 kB\n"[..],
                &b"VmRSS: 1.5 kB\n"[..],
                &b"VmRSS: 1\n"[..],
                &b"VmRSS: 18446744073709551615 kB\n"[..],
                &b"VmRSS: 1 kB\nVmRSS: 2 kB\n"[..],
            ] {
                assert_eq!(parse_rss(invalid), None);
            }
        }
        #[test]
        fn status_read_is_bounded_and_io_failure_is_not_zero_usage() {
            let mut bytes = b"VmRSS: 1 kB\n".to_vec();
            bytes.resize(STATUS_LIMIT, b'x');
            assert_eq!(read_rss(bytes.as_slice()), Some(1024));
            bytes.resize(STATUS_LIMIT * 2, b'x');
            let mut oversized = std::io::Cursor::new(bytes);
            assert_eq!(read_rss(&mut oversized), None);
            assert_eq!(oversized.position(), (STATUS_LIMIT + 1) as u64);
            struct Broken;
            impl Read for Broken {
                fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                    Err(std::io::Error::other("synthetic failure"))
                }
            }
            assert_eq!(read_rss(Broken), None);
        }
        #[test]
        fn own_process_clock_is_monotonic_and_procfs_is_readable() {
            let first = cpu_time().unwrap();
            let mut number = 0_u64;
            for value in 0..10000 {
                number = std::hint::black_box(number.wrapping_add(value));
            }
            std::hint::black_box(number);
            assert!(cpu_time().unwrap() >= first);
            assert!(resident_bytes().unwrap() > 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn cancelled_sampler_never_collects_and_running_sampler_joins_cleanly() {
        let telemetry = Arc::new(RuntimeTelemetry::default());
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        run(telemetry.clone(), cancelled).await;
        assert!(telemetry.snapshot().process.is_none());
        let stop = CancellationToken::new();
        let task = tokio::spawn(run(telemetry.clone(), stop.clone()));
        for _ in 0..10 {
            if telemetry.snapshot().process.is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
        let first = telemetry.snapshot().process.expect("first probe");
        stop.cancel();
        task.await.unwrap();
        tokio::time::advance(Duration::from_secs(10)).await;
        assert_eq!(
            telemetry.snapshot().process.unwrap().observed_at,
            first.observed_at
        );
        assert_eq!(Arc::strong_count(&telemetry), 1);
    }
}
