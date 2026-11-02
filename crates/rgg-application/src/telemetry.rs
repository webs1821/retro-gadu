// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Volatile, fixed-cardinality process observations. Never a durable delivery ledger.
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

macro_rules! categories {
    ($name:ident, $($variant:ident => $label:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug)]
        #[repr(usize)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn label(self) -> &'static str { match self { $(Self::$variant => $label),+ } }
        }
    };
}
categories!(Operation, Login => "login", Accept => "accept", Delivery => "delivery");
categories!(Outcome,
    Success => "success", Rejected => "rejected", Quota => "quota", Invalid => "invalid",
    Timeout => "timeout", Transport => "transport", Stale => "stale", Busy => "busy",
    Storage => "storage", Integrity => "integrity", NotReady => "not_ready",
    Cancelled => "cancelled", Other => "other",
);
categories!(Admission,
    Accepted => "accepted", AddressCapacity => "address_capacity", Rate => "rate",
    GlobalLimit => "global_limit", IpLimit => "ip_limit",
);
categories!(ConnectionEnd,
    Finished => "finished", Shutdown => "shutdown", Replaced => "replaced",
    PeerClosed => "peer_closed", Protocol => "protocol", Timeout => "timeout",
    Storage => "storage", Rate => "rate", Io => "io", Stale => "stale",
    Other => "other", Cancelled => "cancelled",
);

pub const BUCKET_NANOS: [u128; 9] = [
    1_000_000,
    5_000_000,
    10_000_000,
    50_000_000,
    100_000_000,
    500_000_000,
    1_000_000_000,
    5_000_000_000,
    15_000_000_000,
];
pub const BUCKET_LABELS: [&str; 10] = [
    "0.001", "0.005", "0.01", "0.05", "0.1", "0.5", "1", "5", "15", "+Inf",
];

#[derive(Clone, Copy, Debug, Default)]
pub struct Histogram {
    /// Disjoint buckets; conversion to cumulative counts happens on a copied snapshot.
    pub buckets: [u64; 10],
    pub sum_nanos: u128,
}
impl Histogram {
    pub fn cumulative(&self) -> [u64; 10] {
        let mut sum = 0u64;
        self.buckets.map(|value| {
            sum = sum.saturating_add(value);
            sum
        })
    }
    fn observe(&mut self, elapsed: Duration) {
        let nanos = elapsed.as_nanos();
        let bucket = BUCKET_NANOS
            .iter()
            .position(|limit| nanos <= *limit)
            .unwrap_or(9);
        self.buckets[bucket] = self.buckets[bucket].saturating_add(1);
        self.sum_nanos = self.sum_nanos.saturating_add(nanos);
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    pub process: Option<ProcessSample>,
    pub active_connections: u64,
    pub admissions: [u64; Admission::ALL.len()],
    pub endings: [u64; ConnectionEnd::ALL.len()],
    pub outcomes: [[u64; Outcome::ALL.len()]; Operation::ALL.len()],
    pub latency: [Histogram; Operation::ALL.len()],
}
#[derive(Clone, Copy, Debug)]
pub struct ProcessSample {
    pub observed_at: Instant,
    pub cpu_time: Option<Duration>,
    pub resident_bytes: Option<u64>,
}
#[derive(Default)]
pub struct RuntimeTelemetry {
    // No I/O, allocation, or await under this lock; copying all fields gives a coherent sample.
    state: Mutex<Snapshot>,
}
impl RuntimeTelemetry {
    pub fn snapshot(&self) -> Snapshot {
        *self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// The caller collects OS observations before taking this short lock.
    pub fn process_sample(&self, sample: ProcessSample) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).process = Some(sample);
    }
    pub fn admission(&self, result: Admission) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let count = &mut state.admissions[result as usize];
        *count = count.saturating_add(1);
    }
    pub fn connection(self: &Arc<Self>) -> ConnectionObservation {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.active_connections = state.active_connections.saturating_add(1);
        ConnectionObservation {
            telemetry: self.clone(),
            end: ConnectionEnd::Cancelled,
        }
    }
    pub fn operation(self: &Arc<Self>, operation: Operation) -> Observation {
        Observation {
            telemetry: self.clone(),
            operation,
            started: Instant::now(),
            finished: false,
        }
    }
    fn record(&self, operation: Operation, outcome: Outcome, elapsed: Duration) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        // Freeze this operation at the count limit so result totals and +Inf remain equal.
        if state.latency[operation as usize].cumulative()[9] == u64::MAX {
            return;
        }
        let count = &mut state.outcomes[operation as usize][outcome as usize];
        *count = count.saturating_add(1);
        state.latency[operation as usize].observe(elapsed);
    }
}
#[must_use]
pub struct Observation {
    telemetry: Arc<RuntimeTelemetry>,
    operation: Operation,
    started: Instant,
    finished: bool,
}
impl Observation {
    /// A completed delivery poll returned no lease. Failures and cancellation must be recorded.
    pub fn discard_empty_poll(mut self) {
        self.finished = true;
    }
    /// Exactly one observation even when several error paths inspect the same failure.
    pub fn finish(&mut self, result: Outcome) {
        if !self.finished {
            self.finished = true;
            self.telemetry
                .record(self.operation, result, self.started.elapsed());
        }
    }
}
impl Drop for Observation {
    fn drop(&mut self) {
        self.finish(Outcome::Cancelled);
    }
}
#[must_use]
pub struct ConnectionObservation {
    telemetry: Arc<RuntimeTelemetry>,
    end: ConnectionEnd,
}
impl ConnectionObservation {
    pub fn finish(&mut self, end: ConnectionEnd) {
        self.end = end;
    }
}
impl Drop for ConnectionObservation {
    fn drop(&mut self) {
        let mut state = self
            .telemetry
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        state.active_connections = state.active_connections.saturating_sub(1);
        let count = &mut state.endings[self.end as usize];
        *count = count.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_boundaries_cumulative_infinity_and_saturating_arithmetic() {
        let telemetry = RuntimeTelemetry::default();
        for nanos in [0, 1_000_000, 1_000_001, 15_000_000_000, 15_000_000_001] {
            telemetry.record(
                Operation::Accept,
                Outcome::Success,
                Duration::from_nanos(nanos),
            );
        }
        let sample = telemetry.snapshot();
        assert_eq!(
            sample.latency[1].cumulative(),
            [2, 3, 3, 3, 3, 3, 3, 3, 4, 5]
        );
        assert_eq!(sample.latency[1].sum_nanos, 30_002_000_002);
        assert_eq!(sample.outcomes[1][0], 5);
        let mut h = Histogram {
            buckets: [u64::MAX; 10],
            sum_nanos: u128::MAX,
        };
        h.observe(Duration::MAX);
        assert_eq!(h.cumulative(), [u64::MAX; 10]);
        assert_eq!(h.sum_nanos, u128::MAX);
        {
            let mut state = telemetry.state.lock().unwrap();
            state.outcomes[0][0] = u64::MAX - 1;
            state.latency[0].buckets[0] = u64::MAX - 1;
        }
        telemetry.record(Operation::Login, Outcome::Rejected, Duration::from_nanos(1));
        telemetry.record(Operation::Login, Outcome::Rejected, Duration::from_nanos(1));
        let sample = telemetry.snapshot();
        assert_eq!(sample.outcomes[0][1], 1);
        assert_eq!(sample.latency[0].cumulative()[9], u64::MAX);
        assert_eq!(sample.latency[0].sum_nanos, 1);
    }
    #[test]
    fn cancellation_drop_and_duplicate_finish_are_accounted_once() {
        let telemetry = Arc::new(RuntimeTelemetry::default());
        let mut connection = telemetry.connection();
        assert_eq!(telemetry.snapshot().active_connections, 1);
        let mut observed = telemetry.operation(Operation::Login);
        observed.finish(Outcome::Success);
        observed.finish(Outcome::Other);
        drop(observed);
        drop(telemetry.operation(Operation::Delivery));
        telemetry
            .operation(Operation::Delivery)
            .discard_empty_poll();
        connection.finish(ConnectionEnd::Shutdown);
        drop(connection);
        let sample = telemetry.snapshot();
        assert_eq!(sample.active_connections, 0);
        assert_eq!(sample.endings[ConnectionEnd::Shutdown as usize], 1);
        assert_eq!(sample.outcomes[0][0], 1);
        assert_eq!(sample.latency[0].cumulative()[9], 1);
        assert_eq!(sample.outcomes[2][Outcome::Cancelled as usize], 1);
        assert_eq!(sample.latency[2].cumulative()[9], 1);
        assert!(std::mem::size_of::<Snapshot>() < 2048);
    }
    #[test]
    fn concurrent_updates_and_snapshots_keep_histogram_counts_consistent() {
        let telemetry = Arc::new(RuntimeTelemetry::default());
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let telemetry = telemetry.clone();
                scope.spawn(move || {
                    for _ in 0..1000 {
                        telemetry.record(
                            Operation::Accept,
                            Outcome::Success,
                            Duration::from_micros(10),
                        );
                        let s = telemetry.snapshot();
                        assert_eq!(s.outcomes[1][0], s.latency[1].cumulative()[9]);
                    }
                });
            }
        });
        assert_eq!(telemetry.snapshot().outcomes[1][0], 4000);
    }
}
