use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

struct Bulkhead {
    max_concurrent: u32,
    active: AtomicU32,
    name: String,
}

impl Bulkhead {
    fn new(name: &str, max_concurrent: u32) -> Self {
        Self {
            max_concurrent,
            active: AtomicU32::new(0),
            name: name.to_string(),
        }
    }

    fn try_acquire(&self) -> Option<BulkheadPermit<'_>> {
        let current = self.active.load(Ordering::SeqCst);
        if current >= self.max_concurrent {
            return None;
        }
        match self.active.compare_exchange(
            current,
            current + 1,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => Some(BulkheadPermit { bulkhead: self }),
            Err(_) => None,
        }
    }

    fn active_count(&self) -> u32 {
        self.active.load(Ordering::SeqCst)
    }

    fn release(&self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

struct BulkheadPermit<'a> {
    bulkhead: &'a Bulkhead,
}

impl Drop for BulkheadPermit<'_> {
    fn drop(&mut self) {
        self.bulkhead.release();
    }
}

struct BulkheadPool {
    bulkheads: Vec<Arc<Bulkhead>>,
}

impl BulkheadPool {
    fn new(partitions: Vec<(&str, u32)>) -> Self {
        let bulkheads = partitions
            .into_iter()
            .map(|(name, max)| Arc::new(Bulkhead::new(name, max)))
            .collect();
        Self { bulkheads }
    }

    fn least_loaded(&self) -> Option<Arc<Bulkhead>> {
        self.bulkheads
            .iter()
            .min_by_key(|b| b.active_count())
            .cloned()
    }
}

fn main() {
    let pool = Arc::new(BulkheadPool::new(vec![
        ("api", 3),
        ("database", 5),
        ("cache", 2),
    ]));

    println!("Bulkhead Pool:");
    for bh in &pool.bulkheads {
        println!("  {} → max={}", bh.name, bh.max_concurrent);
    }

    // Simulate concurrent access
    let mut handles = vec![];
    for i in 0..10 {
        let pool = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            let bh = pool.least_loaded().unwrap();
            match bh.try_acquire() {
                Some(_permit) => {
                    println!("Task {} → acquired {} (active={})", i, bh.name, bh.active_count());
                    thread::sleep(Duration::from_millis(50));
                    println!("Task {} → released {}", i, bh.name);
                }
                None => {
                    println!("Task {} → rejected from {} (full)", i, bh.name);
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    println!("\nFinal state:");
    for bh in &pool.bulkheads {
        println!("  {}: active={}", bh.name, bh.active_count());
    }
}
