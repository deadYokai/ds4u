use std::sync::{Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

#[inline]
pub fn mlock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[inline]
pub fn rlock<T>(m: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    m.read().unwrap_or_else(|e| e.into_inner())
}

#[inline]
pub fn wlock<T>(m: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    m.write().unwrap_or_else(|e| e.into_inner())
}

#[inline]
pub fn wait_cv<'a, T>(
    cv: &Condvar,
    guard: MutexGuard<'a, T>,
    timeout: Duration,
) -> (MutexGuard<'a, T>, bool) {
    match cv.wait_timeout(guard, timeout) {
        Ok((g, r)) => (g, r.timed_out()),
        Err(e) => (e.into_inner().0, false),
    }
}
