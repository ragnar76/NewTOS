/*
 * lp64-limits.c -- compile-time check of the integer limit macros of the
 * NewTOS libc headers against the real AArch64/LP64 types (TASK/SDK01 A,
 * docs/analysis/sdk-inventory-2026-10-20.md W5).
 *
 * Compiled by lp64-limits.sh for the TARGET (clang -target
 * aarch64-unknown-none-elf, the include path and DEFS of build-libc.sh), so
 * the types are the ones a NewTOS program gets, whatever the host. Every
 * check is a _Static_assert: the file compiles iff all limits match their
 * type. With mintlib's pristine include/stdint.h and bits/posix1_lim.h
 * (LIMITS_HEADER_DIR red run) 10 assertions fail: SIZE_MAX, PTRDIFF_MIN/MAX
 * and SSIZE_MAX (SDK01 A) and INTPTR_MIN/MAX and the uintptr_t width (the
 * PORT10 part of the same overlay).
 */
#include <stddef.h>
#include <stdint.h>
#include <limits.h>
#include <signal.h>
#include <sys/types.h>

#define SAME_TYPE(a, b) _Generic((a), b: 1, default: 0)

/* size_t / ptrdiff_t / ssize_t: the limits SDK01 A fixed. */
_Static_assert(SIZE_MAX == (size_t)-1, "SIZE_MAX != (size_t)-1");
_Static_assert(sizeof(SIZE_MAX) == sizeof(size_t), "SIZE_MAX width");
_Static_assert(PTRDIFF_MAX == __PTRDIFF_MAX__, "PTRDIFF_MAX");
_Static_assert(PTRDIFF_MIN == -__PTRDIFF_MAX__ - 1, "PTRDIFF_MIN");
_Static_assert(sizeof(PTRDIFF_MAX) == sizeof(ptrdiff_t), "PTRDIFF_MAX width");
_Static_assert(sizeof(ssize_t) == sizeof(long), "ssize_t is long");
_Static_assert(SSIZE_MAX == (ssize_t)(((size_t)-1) >> 1), "SSIZE_MAX");
_Static_assert(sizeof(SSIZE_MAX) == sizeof(ssize_t), "SSIZE_MAX width");

/* Neighbours that were already right -- kept so a regression shows. */
_Static_assert(LONG_MAX == __LONG_MAX__, "LONG_MAX");
_Static_assert(ULONG_MAX == (unsigned long)-1, "ULONG_MAX");
_Static_assert(INT_MAX == __INT_MAX__, "INT_MAX");
_Static_assert(UINT_MAX == (unsigned int)-1, "UINT_MAX");
_Static_assert(INTPTR_MAX == __INTPTR_MAX__, "INTPTR_MAX");
_Static_assert(INTPTR_MIN == -__INTPTR_MAX__ - 1, "INTPTR_MIN");
_Static_assert(UINTPTR_MAX == (uintptr_t)-1, "UINTPTR_MAX");
_Static_assert(sizeof(uintptr_t) == sizeof(void *), "uintptr_t width");
_Static_assert(INTMAX_MAX == __INTMAX_MAX__, "INTMAX_MAX");
_Static_assert(UINTMAX_MAX == (uintmax_t)-1, "UINTMAX_MAX");
_Static_assert(INT64_MAX == 9223372036854775807LL, "INT64_MAX");
_Static_assert(UINT64_MAX == (uint64_t)-1, "UINT64_MAX");
_Static_assert(UINT32_MAX == (uint32_t)-1, "UINT32_MAX");
_Static_assert(SIG_ATOMIC_MAX == (sig_atomic_t)(((unsigned)-1) >> 1) ||
               sizeof(sig_atomic_t) != sizeof(int), "SIG_ATOMIC_MAX");

int lp64_limits_ok(void) { return SAME_TYPE(SIZE_MAX, unsigned long); }
