//! Runs a function over a slice on several threads.
//!
//! The results come back in the order of the items.
//! Each thread takes the next item that nobody has taken, so a long item does not hold the others.
//! `DKT_JOBS` sets the number of threads, which is otherwise the number of cores up to `MAX_JOBS`.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// The most threads that run at once by default.
/// Past it the time stops falling while each thread keeps the memory of a document.
const MAX_JOBS: usize = 8;

/// The results of `f` over `items`, in the order of the items.
pub fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let workers = std::env::var("DKT_JOBS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| thread::available_parallelism().map_or(1, |n| n.get().min(MAX_JOBS)))
        .min(items.len());
    if workers <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<R>>> = items.iter().map(|_| Mutex::new(None)).collect();
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    *slots[i].lock().unwrap() = Some(f(item));
                }
            });
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.into_inner().unwrap().unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_results_keep_the_order_of_the_items() {
        let items: Vec<u32> = (0..200).collect();
        assert_eq!(
            map(&items, |n| n * 2),
            items.iter().map(|n| n * 2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn no_item_gives_no_result() {
        let items: Vec<u32> = Vec::new();
        assert!(map(&items, |n| *n).is_empty());
    }
}
