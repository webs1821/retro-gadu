// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Fixed-cardinality, read-only observations of ephemeral image transfers.
use rgg_application::images::{ImageSnapshot, TRANSFER_STATES};
use std::{fmt::Write, time::Instant};

pub(crate) fn append(out: &mut String, sample: Option<ImageSnapshot>, now: Instant) {
    fn header(out: &mut String, name: &str, help: &str) {
        writeln!(out, "# HELP {name} {help}\n# TYPE {name} gauge").expect("String output");
    }
    header(
        out,
        "rgg_image_measurements_available",
        "Whether a nonblocking coordinator snapshot is available; zero when disabled, busy or poisoned.",
    );
    writeln!(
        out,
        "rgg_image_measurements_available {}",
        u8::from(sample.is_some())
    )
    .expect("String output");
    header(
        out,
        "rgg_image_transfers",
        "Coordinator entries by state, including missing notifications awaiting removal.",
    );
    if let Some(sample) = sample {
        for (state, count) in TRANSFER_STATES.into_iter().zip(sample.transfers) {
            writeln!(out, "rgg_image_transfers{{state=\"{state}\"}} {count}")
                .expect("String output");
        }
    }
    for (name, help, value) in [
        (
            "rgg_image_reserved_bytes",
            "Declared image sizes reserved by non-missing transfers, not allocated memory.",
            sample.map(|s| s.reserved_bytes),
        ),
        (
            "rgg_image_buffered_bytes",
            "Length of image and filename buffers owned by the coordinator; excludes outgoing chunks.",
            sample.map(|s| s.buffered_bytes),
        ),
        (
            "rgg_image_buffer_capacity_bytes",
            "Capacity of image and filename buffers owned by the coordinator; excludes allocator overhead and outgoing chunks.",
            sample.map(|s| s.buffer_capacity_bytes),
        ),
    ] {
        header(out, name, help);
        if let Some(value) = value {
            writeln!(out, "{name} {value}").expect("String output");
        }
    }
    header(
        out,
        "rgg_image_oldest_transfer_age_seconds",
        "Monotonic age of the oldest non-missing transfer; zero when none or timestamp is in the future. Scraping does not expire transfers.",
    );
    if let Some(sample) = sample {
        let age = sample
            .oldest_started
            .and_then(|started| now.checked_duration_since(started))
            .unwrap_or_default();
        writeln!(
            out,
            "rgg_image_oldest_transfer_age_seconds {:.9}",
            age.as_secs_f64()
        )
        .expect("String output");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn unavailable_empty_partial_and_extreme_snapshots_are_distinct_and_bounded() {
        let now = Instant::now();
        let render = |sample, now| {
            let mut out = String::new();
            append(&mut out, sample, now);
            assert_eq!(out.matches("# TYPE ").count(), 6);
            assert!(out.is_ascii() && out.len() < 2048);
            out
        };
        let unavailable = render(None, now);
        assert_eq!(
            unavailable
                .lines()
                .filter(|line| !line.starts_with('#'))
                .count(),
            1
        );
        assert!(unavailable.contains("\nrgg_image_measurements_available 0\n"));
        let empty = render(Some(ImageSnapshot::default()), now);
        assert_eq!(
            empty.lines().filter(|line| !line.starts_with('#')).count(),
            9
        );
        assert!(empty.contains("\nrgg_image_measurements_available 1\n"));
        assert!(empty.contains("\nrgg_image_oldest_transfer_age_seconds 0.000000000\n"));
        let sample = ImageSnapshot {
            transfers: [1, 2, 3, 4],
            reserved_bytes: u64::MAX,
            buffered_bytes: u64::MAX,
            buffer_capacity_bytes: u64::MAX,
            oldest_started: Some(now),
        };
        let extreme = render(Some(sample), now + Duration::from_millis(32500));
        assert!(extreme.contains("\nrgg_image_oldest_transfer_age_seconds 32.500000000\n"));
        assert!(extreme.contains("rgg_image_transfers{state=\"missing\"} 4\n"));
        let future = render(
            Some(ImageSnapshot {
                oldest_started: Some(now + Duration::from_secs(1)),
                ..sample
            }),
            now,
        );
        assert!(future.contains("\nrgg_image_oldest_transfer_age_seconds 0.000000000\n"));
    }
}
