use std::sync::Mutex;
use std::time::Instant;

/// Simple token-bucket rate limiter for destructive API endpoints.
/// Allows `capacity` burst requests, refilling at `refill_per_sec` tokens/second.
pub struct RateLimiter {
    inner: Mutex<Bucket>,
}

struct Bucket {
    tokens: f64,
    capacity: f64,
    refill_per_sec: f64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(capacity: u32, refill_per_sec: f64) -> Self {
        Self {
            inner: Mutex::new(Bucket {
                tokens: capacity as f64,
                capacity: capacity as f64,
                refill_per_sec,
                last_refill: Instant::now(),
            }),
        }
    }

    /// Try to consume one token. Returns true if allowed.
    pub fn try_acquire(&self) -> bool {
        let mut bucket = self.inner.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * bucket.refill_per_sec).min(bucket.capacity);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}
