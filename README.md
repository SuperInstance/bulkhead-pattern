# bulkhead-pattern

A Rust implementation of the **Bulkhead concurrency isolation pattern** — partitioning resources into isolated pools so that failures in one subsystem cannot exhaust resources needed by others. Named after the watertight compartments (bulkheads) in ship hulls that prevent a single breach from sinking the vessel.

## Why It Matters

The Bulkhead pattern is one of the core **resilience patterns** in distributed systems engineering (alongside circuit breakers, timeouts, and retries). Without bulkheading:

- A slow database query can consume all connection pool slots, starving health checks
- A computationally expensive API endpoint can starve fast endpoints
- One tenant's traffic spike can degrade service for all other tenants

Systems like Netflix Hystrix, resilience4j, and Envoy implement bulkheads for exactly these scenarios. This crate provides a thread-safe, zero-dependency implementation suitable for embedded systems and teaching.

## How It Works

### Resource Isolation

Each `Bulkhead` maintains a bounded concurrent-access counter using lock-free `AtomicU32` operations:

```text
try_acquire():
    loop:
        current = active.load()
        if current >= max_concurrent: return None  // rejected
        CAS(current, current + 1) → success: return Permit
        CAS failed: retry
```

The **compare-and-swap (CAS)** loop ensures thread safety without mutex overhead. Under low contention, acquire is O(1) with zero blocking.

### Permit Lifecycle via RAII

The `BulkheadPermit` releases on `Drop`:

```rust
impl Drop for BulkheadPermit<'_> {
    fn drop(&mut self) {
        self.bulkhead.active.fetch_sub(1, Ordering::SeqCst);
    }
}
```

This guarantees no resource leaks — even if the task panics, the permit is released.

### Load-Balanced Pool

The `BulkheadPool` implements least-loaded routing:

$$\text{target} = \arg\min_{b \in \text{pool}} \text{active}(b)$$

This is the Join-Shortest-Queue (JSQ) policy, which is known to be optimal for homogeneous servers (Winston, 1977).

### Complexity Analysis

| Operation | Time | Space |
|-----------|------|-------|
| `try_acquire()` | O(1) amortized (CAS) | O(1) |
| `release()` (Drop) | O(1) | O(1) |
| `least_loaded()` | O(P) where P = partitions | O(1) |

Under contention, CAS retries add a expected cost of O(1/(1 − ρ)) where ρ is the utilization, but this remains bounded for ρ < 1.

## Quick Start

```rust
use std::sync::Arc;
use bulkhead_pattern::*; // Bulkhead, BulkheadPool

let pool = Arc::new(BulkheadPool::new(vec![
    ("api", 3),
    ("database", 5),
    ("cache", 2),
]));

// Acquire from least-loaded partition
let bh = pool.least_loaded().unwrap();
match bh.try_acquire() {
    Some(_permit) => {
        // Do work — permit auto-releases when dropped
    }
    None => {
        // Partition full — rejected
    }
}
```

## API

| Type | Method | Description |
|------|--------|-------------|
| `Bulkhead` | `try_acquire() → Option<Permit>` | Non-blocking acquire |
| `Bulkhead` | `active_count() → u32` | Current active permits |
| `BulkheadPermit` | `Drop` | Auto-release on scope exit |
| `BulkheadPool` | `least_loaded() → Option<Arc<Bulkhead>>` | JSQ routing |
| `BulkheadPool` | `new(partitions: Vec<(&str, u32)>)` | Create multi-partition pool |

## Architecture Notes

The **γ + η = C** link: the CAS increment (γ) admits new work into a partition, while the Drop-based decrement (η) guarantees release. Together they conserve the invariant C — the active count never exceeds `max_concurrent`, ensuring that a bulkheaded subsystem can never consume more than its allocated share of resources. The least-loaded routing provides soft fairness across partitions.

## References

- Nygard, M. T. (2018). *Release It! Design and Deploy Production-Ready Software,* 2nd ed. Pragmatic Bookshelf. Chapter 5: "Stability Patterns."
- Winston, W. L. (1977). *Optimality of the Shortest Line Discipline.* Journal of Applied Probability, 14(1), 181–189.
- Netflix Hystrix Wiki: *How it Works: Bulkhead.* GitHub.
- resilience4j: *Bulkhead.* <https://resilience4j.readme.io/docs/bulkhead>
- Goetz, B., et al. (2006). *Java Concurrency in Practice.* Addison-Wesley. (Non-blocking algorithms, Chapter 15.)

## License

MIT
