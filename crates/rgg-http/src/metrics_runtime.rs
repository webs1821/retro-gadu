// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Format facts: prometheus-text-format-reference in THIRD_PARTY.toml; own implementation.
use rgg_application::telemetry::{
    Admission, BUCKET_LABELS, ConnectionEnd, Operation, Outcome, Snapshot,
};
use std::fmt::Write;

fn metadata(out: &mut String, name: &str, kind: &str, help: &str) {
    writeln!(out, "# HELP {name} {help}\n# TYPE {name} {kind}").expect("String output");
}
pub(crate) fn append(out: &mut String, sample: Snapshot) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rgg_application::telemetry::Histogram;
    #[test]
    fn maximum_numeric_values_keep_the_export_bounded_and_finite() {
        let sample = Snapshot {
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
}
