// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Format facts: prometheus-text-format-reference in THIRD_PARTY.toml; own implementation.
use rgg_application::telemetry::{
    Admission, BUCKET_LABELS, ConnectionEnd, Operation, Outcome, ProcessSample, Snapshot,
};
use std::{
    fmt::Write,
    time::{Duration, Instant},
};

fn metadata(out: &mut String, name: &str, kind: &str, help: &str) {
    writeln!(out, "# HELP {name} {help}\n# TYPE {name} {kind}").expect("String output");
}
pub(crate) fn append(out: &mut String, sample: Snapshot) {
    append_process(out, sample.process, Instant::now());
    metadata(
        out,
        "rgg_gg_connections",
        "gauge",
        "GG sockets held by started handlers, including unauthenticated clients.",
    );
    writeln!(out, "rgg_gg_connections {}", sample.active_connections).expect("String output");
    metadata(
        out,
        "rgg_gg_admissions_total",
        "counter",
        "GG socket admission results since process start.",
    );
    for result in Admission::ALL {
        writeln!(
            out,
            "rgg_gg_admissions_total{{result=\"{}\"}} {}",
            result.label(),
            sample.admissions[*result as usize]
        )
        .expect("String output");
    }
    metadata(
        out,
        "rgg_gg_connection_ends_total",
        "counter",
        "Released GG socket scopes by observed reason since process start.",
    );
    for reason in ConnectionEnd::ALL {
        writeln!(
            out,
            "rgg_gg_connection_ends_total{{reason=\"{}\"}} {}",
            reason.label(),
            sample.endings[*reason as usize]
        )
        .expect("String output");
    }
    metadata(
        out,
        "rgg_operations_total",
        "counter",
        "Observed transport operation results; delivery success is TCP plus DB recording, never recipient acknowledgement.",
    );
    for operation in Operation::ALL {
        for result in Outcome::ALL {
            writeln!(
                out,
                "rgg_operations_total{{operation=\"{}\",result=\"{}\"}} {}",
                operation.label(),
                result.label(),
                sample.outcomes[*operation as usize][*result as usize]
            )
            .expect("String output");
        }
    }
    metadata(
        out,
        "rgg_operation_duration_seconds",
        "histogram",
        "Monotonic duration of login, application accept, and delivery attempts including claim; empty polls omitted.",
    );
    for operation in Operation::ALL {
        let histogram = sample.latency[*operation as usize];
        let cumulative = histogram.cumulative();
        for (bound, value) in BUCKET_LABELS.iter().zip(cumulative) {
            writeln!(
                out,
                "rgg_operation_duration_seconds_bucket{{operation=\"{}\",le=\"{bound}\"}} {value}",
                operation.label()
            )
            .expect("String output");
        }
        writeln!(
            out,
            "rgg_operation_duration_seconds_sum{{operation=\"{}\"}} {:.9}",
            operation.label(),
            histogram.sum_nanos as f64 / 1_000_000_000.0
        )
        .expect("String output");
        writeln!(
            out,
            "rgg_operation_duration_seconds_count{{operation=\"{}\"}} {}",
            operation.label(),
            cumulative[9]
        )
        .expect("String output");
    }
}

fn append_process(out: &mut String, sample: Option<ProcessSample>, now: Instant) {
    let age = sample.and_then(|value| now.checked_duration_since(value.observed_at));
    let fresh = sample.filter(|_| age.is_some_and(|age| age <= Duration::from_secs(5)));
    let cpu = fresh.and_then(|value| value.cpu_time);
    let resident = fresh.and_then(|value| value.resident_bytes);
    metadata(
        out,
        "rgg_process_measurements_available",
        "gauge",
        "Whether this process measurement is available and at most five seconds old.",
    );
    for (resource, available) in [
        ("cpu", cpu.is_some()),
        ("resident_memory", resident.is_some()),
    ] {
        writeln!(
            out,
            "rgg_process_measurements_available{{resource=\"{resource}\"}} {}",
            u8::from(available)
        )
        .expect("String output");
    }
    metadata(
        out,
        "rgg_process_sample_age_seconds",
        "gauge",
        "Monotonic age of the last process probe; omitted before a valid timestamp exists.",
    );
    if let Some(age) = age {
        writeln!(
            out,
            "rgg_process_sample_age_seconds {:.9}",
            age.as_secs_f64()
        )
        .expect("String output");
    }
    metadata(
        out,
        "rgg_process_cpu_seconds_total",
        "counter",
        "CPU seconds used by all threads of this process, excluding child processes.",
    );
    if let Some(cpu) = cpu {
        writeln!(
            out,
            "rgg_process_cpu_seconds_total {:.9}",
            cpu.as_secs_f64()
        )
        .expect("String output");
    }
    metadata(
        out,
        "rgg_process_resident_memory_bytes",
        "gauge",
        "Approximate resident bytes of this process from Linux procfs, not cgroup memory.",
    );
    if let Some(resident) = resident {
        writeln!(out, "rgg_process_resident_memory_bytes {resident}").expect("String output");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgg_application::telemetry::Histogram;
    #[test]
    fn maximum_numeric_values_keep_the_export_bounded_and_finite() {
        let sample = Snapshot {
            process: Some(ProcessSample {
                observed_at: Instant::now(),
                cpu_time: Some(Duration::MAX),
                resident_bytes: Some(u64::MAX),
            }),
            active_connections: u64::MAX,
            admissions: [u64::MAX; Admission::ALL.len()],
            endings: [u64::MAX; ConnectionEnd::ALL.len()],
            outcomes: [[u64::MAX; Outcome::ALL.len()]; Operation::ALL.len()],
            latency: [Histogram {
                buckets: [u64::MAX; 10],
                sum_nanos: u128::MAX,
            }; 3],
        };
        let mut out = String::new();
        append(&mut out, sample);
        assert!(out.len() < 12 * 1024); // leaves room for the existing DB-gauge families
        assert!(out.is_ascii());
        assert!(!out.contains("NaN") && !out.contains(" Inf") && !out.contains(" inf"));
        assert_eq!(out.matches("le=\"+Inf\"").count(), 3);
    }
    #[test]
    fn missing_stale_and_partial_observations_do_not_invent_zero_usage() {
        let now = Instant::now();
        let sample = ProcessSample {
            observed_at: now,
            cpu_time: Some(Duration::from_millis(1250)),
            resident_bytes: Some(4096),
        };
        let render = |sample, at| {
            let mut out = String::new();
            append_process(&mut out, sample, at);
            out
        };
        let missing = render(None, now);
        assert!(
            missing.contains("resource=\"cpu\"} 0\n")
                && missing.contains("resource=\"resident_memory\"} 0\n")
        );
        assert!(
            !missing.contains("\nrgg_process_cpu_seconds_total ")
                && !missing.contains("\nrgg_process_resident_memory_bytes ")
        );
        assert!(!missing.contains("\nrgg_process_sample_age_seconds "));
        let fresh = render(Some(sample), now + Duration::from_secs(5));
        assert!(fresh.contains("\nrgg_process_cpu_seconds_total 1.250000000\n"));
        assert!(fresh.contains("\nrgg_process_resident_memory_bytes 4096\n"));
        let stale = render(
            Some(sample),
            now + Duration::from_secs(5) + Duration::from_nanos(1),
        );
        assert!(stale.contains("\nrgg_process_sample_age_seconds 5.000000001\n"));
        assert!(
            !stale.contains("\nrgg_process_cpu_seconds_total ")
                && !stale.contains("\nrgg_process_resident_memory_bytes ")
        );
        let partial = render(
            Some(ProcessSample {
                cpu_time: None,
                resident_bytes: Some(0),
                ..sample
            }),
            now,
        );
        assert!(
            partial.contains("resource=\"cpu\"} 0\n")
                && partial.contains("resource=\"resident_memory\"} 1\n")
        );
        assert!(partial.contains("\nrgg_process_resident_memory_bytes 0\n"));
        let future = render(
            Some(ProcessSample {
                observed_at: now + Duration::from_secs(1),
                ..sample
            }),
            now,
        );
        assert!(
            !future.contains("\nrgg_process_sample_age_seconds ")
                && !future.contains("\nrgg_process_cpu_seconds_total ")
        );
        for result in [missing, fresh, stale, partial, future] {
            assert_eq!(result.matches("# TYPE ").count(), 4);
            assert_eq!(
                result
                    .matches("rgg_process_measurements_available{resource=")
                    .count(),
                2
            );
        }
    }
}
