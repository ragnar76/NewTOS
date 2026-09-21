
#ifndef _SETJMP_H
#define _SETJMP_H 1

#ifndef	_FEATURES_H
# include <features.h>
#endif

__BEGIN_DECLS


#if defined(__aarch64__)
/* NewTOS/AArch64 (sdk/libc/newtos-arch/setjmp.S): x19-x28, fp, lr, sp,
   d8-d15, sigmask -- the AAPCS64 callee-saved set instead of the m68k
   retaddr + d2-d7/a2-a7. Slot 21 is the sigmask slot the m68k kept at 52. */
typedef long jmp_buf[22];
#else
typedef long jmp_buf[14]; /* retaddr, 12 regs, sigmask */
#endif

/* Store the calling environment in ENV, also saving the signal mask.
   Return 0.  */
extern int setjmp	(jmp_buf);

/* Jump to the environment saved in ENV, making the
   `setjmp' call there return VAL, or 1 if VAL is 0.  */
extern void longjmp	(jmp_buf, int) __THROWNL __attribute__ ((__noreturn__));


/* Use the same type for `jmp_buf' and `sigjmp_buf'.
   The `__mask_was_saved' flag determines whether
   or not `longjmp' will restore the signal mask.  */
typedef jmp_buf sigjmp_buf;

/* Store the calling environment in ENV, also saving the
   signal mask if SAVEMASK is nonzero.  Return 0.
   This is the internal name for `sigsetjmp'.  */
extern int sigsetjmp  (sigjmp_buf env, int savemask);
extern void siglongjmp (sigjmp_buf env, int val) __THROWNL __attribute__ ((__noreturn__));


#ifdef __USE_BSD
/* Store the calling environment in ENV, not saving the signal mask.
   Return 0.  */
#define _setjmp(__jb)		sigsetjmp(__jb, 0)
#define setjmp(__jb)		sigsetjmp(__jb, 1)

static __inline __attribute__ ((__noreturn__)) void
_longjmp (sigjmp_buf env, int val)
{
	siglongjmp(env, val);
}

#endif /* __USE_BSD */

#ifdef	__USE_POSIX
#undef setjmp
#define setjmp(__jb)		(sigsetjmp(__jb, 1))
#endif

#ifndef	setjmp
#define setjmp(__jb)		(sigsetjmp(__jb, 0))
#endif


__END_DECLS

#endif /* _SETJMP_H */
