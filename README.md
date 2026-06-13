# Bulkhead Pattern

**A Rust library implementing the Bulkhead isolation pattern** — limits concurrent access to resources by partitioning thread pools, preventing one slow service from exhausting connections to others.

## Why It Matters

The Bulkhead pattern is named after ship bulkheads — watertight compartments that prevent a hull breach from sinking the entire vessel. In software, it means isolating resources so that failure in one subsystem doesn't cascade to others.

Without bulkheads, a slow database call can consume all threads in a web server, making it unresponsive to *all* requests — including ones that don't touch the database. With bulkheads, the database calls are limited to a pool of N threads, leaving other threads free to serve health checks, static content, and non-database endpoints.

This is one of three core resilience patterns (alongside circuit breakers and rate limiters). Popularized by Netflix's Hystrix library and codified in Michael Nygard's "Release It!" (2007).

The implementation uses atomic counters for O(1) acquire/release with no lock contention. The RAII `BulkheadPermit` ensures permits are always released, even on panic.

## How It Works

**Bulkhead**: Wraps an `AtomicU32` counter with a max-concurrency limit. `try_acquire()` atomically loads the current count, checks against the limit, and uses `compare_exchange` (CAS) to increment. If the CAS succeeds, it returns a `BulkheadPermit` (RAII guard); if the limit is reached, it returns `None`.

**RAII permit**: `BulkheadPermit` holds a reference to the parent bulkhead. On `Drop` (whether explicit or via panic), it calls `fetch_sub(1)` to release the slot. This is the same pattern Rust uses for `MutexGuard` — compile-time guarantee that resources are freed.

**BulkheadPool**: Groups multiple bulkheads (e.g., "api", "database", "cache") with different limits. `least_loaded()` selects the bulkhead with the fewest active permits — a simple load-balancing strategy.

**Thread safety**: Uses `Arc<Bulkhead>` for shared ownership across threads. `AtomicU32` with `SeqCst` ordering ensures correct visibility across cores.

## Quick Start

```rust
use std::sync::Arc;
use std::thread;

// (From the demo: 10 tasks competing for 3 bulkheads)
// In practice, you'd use the structs directly:
// let bulkhead = Bulkhead::new("database", 5);
// match bulkhead.try_acquire() {
//     Some(_permit) => { /* do work, permit auto-released on drop */ }
//     None => { /* reject or queue the request */ }
// }
```

## API

- **`Bulkhead`** — Concurrency limiter: `new(name, max)`, `try_acquire()` → `Option<Permit>`, `active_count()`
- **`BulkheadPermit`** — RAII guard that releases on drop
- **`BulkheadPool`** — Multi-bulkhead manager: `new(partitions)`, `least_loaded()`

## Architecture Notes

Part of the SuperInstance resilience toolkit alongside `circuit-breaker` and `backpressure-regulator`. The bulkhead is designed to wrap any resource pool (database connections, HTTP clients, thread pools) to enforce isolation boundaries. See the [architecture overview](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md).

## License

MIT
