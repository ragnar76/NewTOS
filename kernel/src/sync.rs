//! Minimal spinlock used by the early kernel.
//!
//! No `spin` / `parking_lot` dependency — the kernel is single-CPU and
//! pre-allocator at the points where this lock is held. The lock
//! exists to make the *API* of allocators thread-safe so we don't have
//! to refactor when SMP arrives.
//!
//! Implementation: ticketless test-and-set with `Acquire`/`Release`
//! ordering. Spins with `wfe` / `sev` to avoid burning power on real
//! hardware. Not fair, not reentrant.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// Test-and-set spinlock around a `T`.
pub struct SpinMutex<T> {
    locked: AtomicBool,
    inner: UnsafeCell<T>,
}

// SAFETY: access to `inner` is gated by `locked`.
unsafe impl<T: Send> Send for SpinMutex<T> {}
unsafe impl<T: Send> Sync for SpinMutex<T> {}

impl<T> SpinMutex<T> {
    /// Construct a new mutex wrapping `value`. `const fn` so it can
    /// initialize `static` mutexes without `OnceCell`.
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            inner: UnsafeCell::new(value),
        }
    }

    /// Acquire the lock, blocking until it is free.
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Wait for an event (SEV from the holder on release) to
            // avoid burning power. On QEMU this is a no-op; on real
            // hardware it lets the CPU enter low-power.
            // SAFETY: `wfe` has no architectural side effects.
            unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) };
        }
        SpinMutexGuard { lock: self }
    }
}

/// RAII guard returned by [`SpinMutex::lock`].
pub struct SpinMutexGuard<'a, T> {
    lock: &'a SpinMutex<T>,
}

impl<'a, T> Deref for SpinMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: we hold the lock for the lifetime of the guard.
        unsafe { &*self.lock.inner.get() }
    }
}

impl<'a, T> DerefMut for SpinMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: we hold the lock for the lifetime of the guard.
        unsafe { &mut *self.lock.inner.get() }
    }
}

impl<'a, T> Drop for SpinMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
        // Wake one waiter (no-op if none).
        // SAFETY: `sev` has no architectural side effects.
        unsafe { core::arch::asm!("sev", options(nomem, nostack, preserves_flags)) };
    }
}
