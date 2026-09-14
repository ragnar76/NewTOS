/*
 * osbind.h	bindings for OS traps
 *
 *		++jrb bammi@cadence.com
 */

/*
 * majorly re-hacked for gcc-1.36 and probably beyond
 * all inlines changed to #defines, beacuse gcc is not
 * handling clobbered reggies correctly when -mshort.
 * We now use the Statement Exprs feature of GnuC
 *
 * 10/12/89
 *	changed all "g" constraints to "r" that will force
 *	all operands to be evaluated (or lea calculated)
 *	before the __asm__. This is necessary because
 *	if we had the (looser) "g" constraint, then sometimes
 *	we are in the situation where stuff is in the stack,
 *	and we are modifying the stack under Gcc (but eventually
 *	restoring it before the end of the __asm__), and it does
 *	not know about it (i believe there is no way to tell it
 *	this either, but you can hardly expect that). by forcing
 *	the stricter "r" constraint, we force the eval before using
 *	the val (or lea as the case may be) and we dont get into
 *	trouble.
 *	(thanks to ers for finding this problem!)
 *	[one side effect of this is that we may(depending on the
 *	  situation) actually end up with better code when the
 *	values are already in reggies, or that value is used
 *	later on (note that Gnu's reggie allocation notices the
 *	clobbered reggie, and does'nt put the value/or uses
 *	them from those reggies, nice huh!)
 *
 *  28/2/90
 *	another major re-hack:
 *	-- the basic reason: there was just no reliable
 *	way to get the definitions (inline or not does'nt matter) to
 *	fully evaluate the args before we changed the sp from under it.
 *	(if -fomit-frame-pointer is *not* used, then most of the time
 *	 we dont need to do this, as things will just reference off of
 *	 a6, but this is not true all of the time).
 *	my solution was to use local vars in the body of the statement
 *	exprs, and initialize them from the args of the statement expr block.
 *	to force the evaluation of the args before we change sp from
 *	under gcc's feet, we make the local vars volatile. we use a
 *	slight code optimization heuristic: if there are more than 4
 *	args, only then we make the local volatile, and relax
 *	the "r" constraint to "g". otherwise, we dont put the volatile
 *	and force the evaluation by putting the "r" constaint. this
 *	produces better code in most sitiations (when !__NO_INLINE__
 *	especially), as either the args are already in a register or
 *	there is good chance they will soon be reused, and in that
 *	case it will already be in a register.
 *      it may (the local vars, especially when no volatile)
 *	look like overhead, but in 99% of the situations gcc will just
 *	optimize that assignment right out. besides, this makes
 *	these defines totally safe (from re-evaluation of the macro args).
 *
 *	-- as suggested by andreas schwab (thanks!)
 *	 (schwab@ls5.informatik.uni-dortmund.de) all the retvalues are now
 *	 local register vals (see the extentions section in the info file)
 *	 this really worked out great as all the silly "movl d0,%0" at
 *	 the end of each def can now be removed, and the value of
 *	 retvalue ends up in the correct register. it avoids all the
 *	 silly "mov d0,[d|a]n" type sequences from being generated. a real win.
 *	 (note in the outputs "=r"(__retvalue) still has to be specified,
 *	 otherwise in certain situations you end up loosing the return
 *	 value in d0, as gcc sees no output, and correctly assumes that the
 *	 asm returns no value).
 *
 *	-- all the n's (the function #'s for the traps) are now given
 *	the more relaxed "g". This again results in better code, as
 *	it is always a constant, and gcc turns the movw %1,sp@- into
 *	a movw #n,sp@-. we could have given them a "i" constraint too,
 *	but "g" gives gcc more breathing room, and it does the right
 *	thing. note: the n's still need to have "r" constraints in the
 *	non-inline form (function form), as they are no longer constants
 *	in the function, but a normal arg on the stack frame, and hence
 *	we need to force evaluation before we change sp. (see osbind.c)
 *
 *	-- straps.cpp and traps.c are history. we dont need no stinking
 *	non-reentrant bindings (straps) or incorrect ones (traps.c :-)
 *
 * 03/15/92 ++jrb
 *	-- another re-hack needed for gcc-2.0: the optimization that we
 *      used earlier for traps with more than 4 args, making them volatile
 *	and using "g" constraints no longer works, because gcc has become
 *	so smart! we now remove the volatile, and give "r" constraints
 *	(just like traps with <= 4 args). that way the args are evaled
 *	before we change the stack under gcc, and at appropriate times
 *	put into reggies and pushed (or as in most cases, they are evaled
 *	straight into reggies and pushed -- and in even more common cases
 *	they are already in reggies, and they are just pushed). not doing
 *	this with -fomit-frame-pointer was causing the temps (from evaluing
 *	the args) to be created on the stack, but when we changed sp
 *	from under gccs feet, the offsets  to the temps ended up being wrong.
 *
 * 10/28/93 ++jrb
 *	relax the constraints on the inputs of trap_14_wwwwwww (only
 *	Rsconf maps to this)  to "g" from "r", as these many "r" 's
 *	give gcc 2.>3.X heartaches (understandably). note this is ok
 *	since these args will never be expressions, and we never
 *	have to constrain hard enough to force eval before we change
 *	sp from underneath gcc.
 *
 */

#ifndef _MINT_OSBIND_H
#define _MINT_OSBIND_H	1

#ifndef _FEATURES_H
# include <features.h>
#endif

#ifndef _MINT_OSTRUCT_H
# include <mint/ostruct.h>
#endif

__BEGIN_DECLS


/*
 * ---------------------------------------------------------------------------
 * NewTOS (AArch64) trap layer -- retargeted from the m68k stack-trap form.
 * ---------------------------------------------------------------------------
 *
 * The original mintlib turned every GEMDOS/BIOS/XBIOS call into a `trap #N`
 * that pushed its arguments right-to-left on the m68k stack (`movw`/`movl`
 * ... %%sp@-) and read the result from d0. NewTOS uses a REGISTER SVC ABI,
 * verified from the kernel dispatcher (kernel/src/arch/aarch64/exceptions.rs,
 * tos-api/src/trap.rs):
 *
 *   - trap class = the SVC immediate: GEMDOS = svc #1, BIOS = svc #13,
 *     XBIOS = svc #14 (matching the m68k trap numbers).
 *   - the function id goes in x8 (kernel reads its low 16 bits).
 *   - integer / pointer arguments go in x0, x1, x2, ... x7 (LEFT TO RIGHT in
 *     declaration order). The kernel captures exactly EIGHT argument
 *     registers x0..x7 (tos_api::trap::TrapArgs::regs is `[u64; 8]`).
 *   - the result comes back in x0 (a sign-extended TOS LONG; BIOS Getbpb
 *     returns a full 64-bit pointer).
 *
 * The kernel SVC entry stub saves and restores x1..x29 across the trap and
 * only overwrites x0 with the result, so -- exactly like sdk/include/newtos.h
 * -- these helpers clobber nothing but memory and the x0 result register; the
 * argument registers are inputs that survive the trap.
 *
 * Consequences for the width chars in the macro names (`w` = 16-bit word,
 * `l` = 32-bit long on m68k): with a register ABI the width no longer selects
 * a stack slot -- EACH argument occupies exactly one 64-bit register and the
 * kernel reads the width it needs (arg_i16 / arg_u16 / arg_i32 / arg_u64).
 * We keep the original per-position `(short)` / `(long)` casts so callers see
 * identical C-visible value semantics: a `w` position is sign-narrowed to 16
 * bits, an `l` position is passed as a full 64-bit long/pointer (the LP64
 * widening of the old ILP32 `long` is deliberate -- pointers are 64-bit here).
 *
 * The macro NAMES and SIGNATURES are unchanged, so all ~200 osbind/mintbind
 * bindings below (and in mintbind.h / falcon.h / metados.h) compile against
 * these helpers with no edits.
 */

/*
 * Arity helpers: `sv` is the raw SVC immediate (1 / 13 / 14), stringized into
 * the instruction; `fn` is the function id (-> x8); a0..a7 are the already
 * width-cast arguments (-> x0..x7). x0 doubles as arg0 input and result
 * output ("+r"); with zero args it is a pure "=r" output. Modelled on the
 * `register long __rN __asm__("xN")` pattern in sdk/include/newtos.h.
 */
#define __newtos_trap0(sv, fn)						\
__extension__								\
({									\
	register long __r8 __asm__("x8") = (long)(fn);			\
	register long __retvalue __asm__("x0");				\
	__asm__ volatile("svc #" #sv					\
		: "=r"(__retvalue)					\
		: "r"(__r8)						\
		: "memory");						\
	__retvalue;							\
})

#define __newtos_trap1(sv, fn, a0)					\
__extension__								\
({									\
	register long __r8 __asm__("x8") = (long)(fn);			\
	register long __retvalue __asm__("x0") = (a0);			\
	__asm__ volatile("svc #" #sv					\
		: "+r"(__retvalue)					\
		: "r"(__r8)						\
		: "memory");						\
	__retvalue;							\
})

#define __newtos_trap2(sv, fn, a0, a1)					\
__extension__								\
({									\
	register long __r8 __asm__("x8") = (long)(fn);			\
	register long __retvalue __asm__("x0") = (a0);			\
	register long __r1 __asm__("x1") = (a1);			\
	__asm__ volatile("svc #" #sv					\
		: "+r"(__retvalue)					\
		: "r"(__r8), "r"(__r1)					\
		: "memory");						\
	__retvalue;							\
})

#define __newtos_trap3(sv, fn, a0, a1, a2)				\
__extension__								\
({									\
	register long __r8 __asm__("x8") = (long)(fn);			\
	register long __retvalue __asm__("x0") = (a0);			\
	register long __r1 __asm__("x1") = (a1);			\
	register long __r2 __asm__("x2") = (a2);			\
	__asm__ volatile("svc #" #sv					\
		: "+r"(__retvalue)					\
		: "r"(__r8), "r"(__r1), "r"(__r2)			\
		: "memory");						\
	__retvalue;							\
})

#define __newtos_trap4(sv, fn, a0, a1, a2, a3)				\
__extension__								\
({									\
	register long __r8 __asm__("x8") = (long)(fn);			\
	register long __retvalue __asm__("x0") = (a0);			\
	register long __r1 __asm__("x1") = (a1);			\
	register long __r2 __asm__("x2") = (a2);			\
	register long __r3 __asm__("x3") = (a3);			\
	__asm__ volatile("svc #" #sv					\
		: "+r"(__retvalue)					\
		: "r"(__r8), "r"(__r1), "r"(__r2), "r"(__r3)		\
		: "memory");						\
	__retvalue;							\
})

#define __newtos_trap5(sv, fn, a0, a1, a2, a3, a4)			\
__extension__								\
({									\
	register long __r8 __asm__("x8") = (long)(fn);			\
	register long __retvalue __asm__("x0") = (a0);			\
	register long __r1 __asm__("x1") = (a1);			\
	register long __r2 __asm__("x2") = (a2);			\
	register long __r3 __asm__("x3") = (a3);			\
	register long __r4 __asm__("x4") = (a4);			\
	__asm__ volatile("svc #" #sv					\
		: "+r"(__retvalue)					\
		: "r"(__r8), "r"(__r1), "r"(__r2), "r"(__r3),		\
		  "r"(__r4)						\
		: "memory");						\
	__retvalue;							\
})

#define __newtos_trap6(sv, fn, a0, a1, a2, a3, a4, a5)			\
__extension__								\
({									\
	register long __r8 __asm__("x8") = (long)(fn);			\
	register long __retvalue __asm__("x0") = (a0);			\
	register long __r1 __asm__("x1") = (a1);			\
	register long __r2 __asm__("x2") = (a2);			\
	register long __r3 __asm__("x3") = (a3);			\
	register long __r4 __asm__("x4") = (a4);			\
	register long __r5 __asm__("x5") = (a5);			\
	__asm__ volatile("svc #" #sv					\
		: "+r"(__retvalue)					\
		: "r"(__r8), "r"(__r1), "r"(__r2), "r"(__r3),		\
		  "r"(__r4), "r"(__r5)					\
		: "memory");						\
	__retvalue;							\
})

#define __newtos_trap7(sv, fn, a0, a1, a2, a3, a4, a5, a6)		\
__extension__								\
({									\
	register long __r8 __asm__("x8") = (long)(fn);			\
	register long __retvalue __asm__("x0") = (a0);			\
	register long __r1 __asm__("x1") = (a1);			\
	register long __r2 __asm__("x2") = (a2);			\
	register long __r3 __asm__("x3") = (a3);			\
	register long __r4 __asm__("x4") = (a4);			\
	register long __r5 __asm__("x5") = (a5);			\
	register long __r6 __asm__("x6") = (a6);			\
	__asm__ volatile("svc #" #sv					\
		: "+r"(__retvalue)					\
		: "r"(__r8), "r"(__r1), "r"(__r2), "r"(__r3),		\
		  "r"(__r4), "r"(__r5), "r"(__r6)			\
		: "memory");						\
	__retvalue;							\
})

#define __newtos_trap8(sv, fn, a0, a1, a2, a3, a4, a5, a6, a7)		\
__extension__								\
({									\
	register long __r8 __asm__("x8") = (long)(fn);			\
	register long __retvalue __asm__("x0") = (a0);			\
	register long __r1 __asm__("x1") = (a1);			\
	register long __r2 __asm__("x2") = (a2);			\
	register long __r3 __asm__("x3") = (a3);			\
	register long __r4 __asm__("x4") = (a4);			\
	register long __r5 __asm__("x5") = (a5);			\
	register long __r6 __asm__("x6") = (a6);			\
	register long __r7 __asm__("x7") = (a7);			\
	__asm__ volatile("svc #" #sv					\
		: "+r"(__retvalue)					\
		: "r"(__r8), "r"(__r1), "r"(__r2), "r"(__r3),		\
		  "r"(__r4), "r"(__r5), "r"(__r6), "r"(__r7)		\
		: "memory");						\
	__retvalue;							\
})

/*
 *     GEMDOS  (trap #1  ->  svc #1)
 */
#define trap_1_w(n)		__newtos_trap0(1, (n))
#define trap_1_ww(n, a)		__newtos_trap1(1, (n), (short)(a))
#define trap_1_wl(n, a)		__newtos_trap1(1, (n), (long)(a))
#define trap_1_wlw(n, a, b)	__newtos_trap2(1, (n), (long)(a), (short)(b))
#define trap_1_www(n, a, b)	__newtos_trap2(1, (n), (short)(a), (short)(b))
#define trap_1_wll(n, a, b)	__newtos_trap2(1, (n), (long)(a), (long)(b))
#define trap_1_wwll(n, a, b, c)	__newtos_trap3(1, (n), (short)(a), (long)(b), (long)(c))
#define trap_1_wlww(n, a, b, c)	__newtos_trap3(1, (n), (long)(a), (short)(b), (short)(c))
#define trap_1_wlwww(n, a, b, c, d)					\
	__newtos_trap4(1, (n), (long)(a), (short)(b), (short)(c), (short)(d))
#define trap_1_wwlll(n, a, b, c, d)					\
	__newtos_trap4(1, (n), (short)(a), (long)(b), (long)(c), (long)(d))
#define trap_1_wwwll(n, a, b, c, d)					\
	__newtos_trap4(1, (n), (short)(a), (short)(b), (long)(c), (long)(d))
#define trap_1_wwllww(n, a, b, c, d, e)					\
	__newtos_trap5(1, (n), (short)(a), (long)(b), (long)(c),	\
		       (short)(d), (short)(e))

/*
 *     BIOS    (trap #13  ->  svc #13)
 */
#define trap_13_w(n)		__newtos_trap0(13, (n))
#define trap_13_ww(n, a)	__newtos_trap1(13, (n), (short)(a))
#define trap_13_wl(n, a)	__newtos_trap1(13, (n), (long)(a))
#define trap_13_www(n, a, b)	__newtos_trap2(13, (n), (short)(a), (short)(b))
#define trap_13_wwl(n, a, b)	__newtos_trap2(13, (n), (short)(a), (long)(b))
#define trap_13_wwlwww(n, a, b, c, d, e)				\
	__newtos_trap5(13, (n), (short)(a), (long)(b), (short)(c),	\
		       (short)(d), (short)(e))
#define trap_13_wwlwwwl(n, a, b, c, d, e, f)				\
	__newtos_trap6(13, (n), (short)(a), (long)(b), (short)(c),	\
		       (short)(d), (short)(e), (long)(f))

/*
 *     XBIOS   (trap #14  ->  svc #14)
 */
#define trap_14_w(n)		__newtos_trap0(14, (n))
#define trap_14_ww(n, a)	__newtos_trap1(14, (n), (short)(a))
#define trap_14_wl(n, a)	__newtos_trap1(14, (n), (long)(a))
#define trap_14_www(n, a, b)	__newtos_trap2(14, (n), (short)(a), (short)(b))
#define trap_14_wwl(n, a, b)	__newtos_trap2(14, (n), (short)(a), (long)(b))
#define trap_14_wwll(n, a, b, c)					\
	__newtos_trap3(14, (n), (short)(a), (long)(b), (long)(c))
#define trap_14_wllw(n, a, b, c)					\
	__newtos_trap3(14, (n), (long)(a), (long)(b), (short)(c))
#define trap_14_wlll(n, a, b, c)					\
	__newtos_trap3(14, (n), (long)(a), (long)(b), (long)(c))
#define trap_14_wwwl(n, a, b, c)					\
	__newtos_trap3(14, (n), (short)(a), (short)(b), (long)(c))
#define trap_14_wllww(n, a, b, c, d)					\
	__newtos_trap4(14, (n), (long)(a), (long)(b), (short)(c), (short)(d))
#define trap_14_wwwwl(n, a, b, c, d)					\
	__newtos_trap4(14, (n), (short)(a), (short)(b), (short)(c), (long)(d))
#define trap_14_wlwlw(n, a, b, c, d)					\
	__newtos_trap4(14, (n), (long)(a), (short)(b), (long)(c), (short)(d))
#define trap_14_wwwwwww(n, a, b, c, d, e, f)				\
	__newtos_trap6(14, (n), (short)(a), (short)(b), (short)(c),	\
		       (short)(d), (short)(e), (short)(f))
#define trap_14_wllwwwww(n, a, b, c, d, e, f, g)			\
	__newtos_trap7(14, (n), (long)(a), (long)(b), (short)(c),	\
		       (short)(d), (short)(e), (short)(f), (short)(g))
#define trap_14_wllwwwwlw(n, a, b, c, d, e, f, g, h)			\
	__newtos_trap8(14, (n), (long)(a), (long)(b), (short)(c),	\
		       (short)(d), (short)(e), (short)(f), (long)(g),	\
		       (short)(h))

/*
 * trap_14_wllwwwwwlw -- 9-argument XBIOS (Flopfmt, XBIOS 0x0A). NewTOS reads
 * only EIGHT argument registers (x0..x7; x8 holds the function id), so a 9th
 * argument has no register slot -- and, more fundamentally, NewTOS has NO
 * floppy hardware. Per the floppy/DMA stub decision (user 2026-07-23, spec
 * docs/api/xbios/_floppy-stubs.md), the whole floppy/DMA XBIOS group returns
 * the contract value EUNDEV (-15) WITHOUT reading its arguments in the kernel,
 * and Flopfmt is deliberately given NO 9th trap-ABI register slot. So this
 * binding must NOT issue an svc: a normal 8-register svc would silently
 * truncate / mis-pass the 9th argument. Instead it evaluates every argument
 * for its C side-effects (each is a caller expression) and yields EUNDEV (-15)
 * directly as a long. The macro signature is kept so Flopfmt() at osbind.h:633
 * still expands correctly through it.
 */
#define trap_14_wllwwwwwlw(n, a, b, c, d, e, f, g, h, i)		\
__extension__								\
({									\
	(void)(n); (void)(a); (void)(b); (void)(c); (void)(d);		\
	(void)(e); (void)(f); (void)(g); (void)(h); (void)(i);		\
	(long)(-15);		/* EUNDEV -- no floppy device on NewTOS */\
})


/* DEFINITIONS FOR OS FUNCTIONS */

/*
 *     GEMDOS  (trap1)
 */
#define	       Pterm0()					       	       \
       (void)trap_1_w((short)(0x00))
#define	       Cconin()						       \
       (long)trap_1_w((short)(0x01))
#define	       Cconout(c)					       \
       (void)trap_1_ww((short)(0x02),(short)(c))
#define	       Cauxin()						       \
       (short)trap_1_w((short)(0x03))
#define	       Cauxout(c)					       \
       (void)trap_1_ww((short)(0x04),(short)(c))
#define	       Cprnout(c)					       \
       (short)trap_1_ww((short)(0x05),(short)(c))
#define	       Crawio(data)					       \
       (long)trap_1_ww((short)(0x06),(short)(data))
#define	       Crawcin()					       \
       (long)trap_1_w((short)(0x07))
#define	       Cnecin()						       \
       (long)trap_1_w((short)(0x08))
#define	       Cconws(s)					       \
       (short)trap_1_wl((short)(0x09),(long)(s))
#define	       Cconrs(buf)					       \
       trap_1_wl((short)(0x0A),(long)(buf))
#define	       Cconis()						       \
       (short)trap_1_w((short)(0x0B))
#define	       Dsetdrv(d)					       \
       (long)trap_1_ww((short)(0x0E),(short)(d))
#define	       Cconos()						       \
       (short)trap_1_w((short)(0x10))
#define	       Cprnos()						       \
       (short)trap_1_w((short)(0x11))
#define	       Cauxis()						       \
       (short)trap_1_w((short)(0x12))
#define	       Cauxos()						       \
       (short)trap_1_w((short)(0x13))
#define	       Dgetdrv()					       \
       (short)trap_1_w((short)(0x19))
#define	       Fsetdta(dta)					       \
       (void)trap_1_wl((short)(0x1A),(long)(dta))

/*
 * ---------------------------------------------------------------------------
 * Super (GEMDOS 0x20) -- NewTOS/AArch64. See TASK/X01.
 * ---------------------------------------------------------------------------
 *
 * The original m68k binding was `trap_1_wl(0x20, ptr)` under this header note:
 *
 *	1. Super(1L) from either user or supervisor mode
 *	2. ret = Super(0L) from user mode and after this Super(ret) from
 *	   supervisor mode
 *	Parameter	Userstack	Superstack	Calling Mode	ret
 *	   1L		   usp		   ssp		    user	 0L
 *	   1L		   usp		   ssp		 supervisor	-1L
 *	   0L		  usp-6		   usp		    user	ssp
 *	   0L		   ssp		  ssp-6		 supervisor   ssp-6
 *	  ptr		  usp-6		  ptr+6		    user	ssp
 *	  ptr		  usp+6		   ptr		 supervisor	 sr
 *
 * WHERE THAT TABLE COMES FROM. Super is not an ordinary BDOS function: EmuTOS
 * implements it in the TRAP #1 FRONT END, in assembly, because it rewrites the
 * exception frame. `bdos/rwa.S:159-182` (`_enter`) compares the function word
 * against 0x20 BEFORE the BDOS table is consulted and branches to one of three
 * handlers -- `x20_usr` (:277-296), `x20_sup` (:300-330), `x20_inq` (:334-339).
 * The `{ NI, 0, 0 }` slot for 0x20 in `bdos/bdosmain.c:221` is therefore DEAD
 * CODE and never reached; "EmuTOS does not implement Super" is wrong.
 *
 * WHAT THE THREE HANDLERS ACTUALLY DO:
 *   x20_inq (parameter == 1L, either mode): changes nothing, and answers the
 *       question "am I privileged?" -- 0L for a user-mode caller, -1L for a
 *       supervisor-mode caller (`btst.b #5,(sp)`).
 *   x20_usr (user mode, parameter != 1L): makes the new SSP either the current
 *       USP (parameter == 0L) or the supplied pointer, moves the return address
 *       and the SR across, sets the supervisor bit, and returns the OLD SSP
 *       (`move.l sp,d0`, :291). The return value is the token the caller must
 *       hand back to undo the switch.
 *   x20_sup (supervisor mode, parameter != 1L): the reverse -- rebuilds a USP
 *       and clears the supervisor bit. It never writes d0, so the "ret" column
 *       above is the caller's own leftover register for those two rows: the
 *       closing half of the bracket has NO defined return value, and no caller
 *       reads one.
 *
 * WHAT THIS MEANS ON AArch64. NewTOS runs applications at EL0 and cannot raise
 * them to EL1: the kernel's page tables, its stack and its vectors are not the
 * application's, and the pointer would come from the application. So there is
 * no mode to enter -- and, crucially, none is needed. Everything the callers
 * actually want lies in the classic $400 system-variable area, and NewTOS
 * materialises that area in the PROCESS's OWN address space, user-readable and
 * user-writable (tos-api/src/bdos/process/loader.rs `populate_sysvars`;
 * docs/analysis/bios-audit/b1-lowmem-feasibility.md, Option 1). Every one of
 * the nine call sites in our ported applications -- TeraDesk
 * library/utility/gettos.c:40-42 ($4F2), main.c:2090, file.c:945/969 (the
 * hdv_* vectors), video.c:289/318 -- is a bracket around a low-memory access,
 * not a request for privilege.
 *
 * THE SIX CASES, RESOLVED FOR EL0:
 *   1L / user       -> 0L.  Unchanged and truthful: an EL0 caller is not
 *                     privileged. Programs branch on this ("do I still need to
 *                     call Super(0)?"), so it must stay honest.
 *   1L / supervisor -> -1L. UNREACHABLE: code that expands this macro runs at
 *                     EL0 by construction, so it collapses into case 1.
 *   0L / user       -> no-op; must return a TOKEN, because the caller feeds it
 *                     straight back in (`stack = Super(NULL); ...;
 *                     Super(stack);`). The token must be neither 0 (which would
 *                     re-open the bracket) nor 1 (which would be read as the
 *                     inquiry). We return the caller's own stack pointer: on
 *                     AArch64 an EL0 thread has exactly one stack (SP_EL0) and
 *                     the EL1 stack is not addressable from here, so "the stack
 *                     pointer that was in force" IS SP_EL0 -- the same thing
 *                     x20_usr hands back, minus a privilege level that does not
 *                     exist. It is naturally neither 0 nor 1, and it keeps the
 *                     callers' `void *stack` variables type-honest.
 *   0L / supervisor -> UNREACHABLE (see above).
 *   ptr / user      -> UNREACHABLE in the bracket idiom: the pointer can only
 *                     have come from a preceding Super(0L), which on NewTOS
 *                     left us at EL0. Treated as the next case.
 *   ptr / supervisor-> the CLOSING half of the bracket: no-op. EmuTOS leaves no
 *                     defined return value here and no caller reads one; we
 *                     return the same stack pointer for symmetry.
 *
 * So the whole function collapses to: report "not privileged" for the inquiry,
 * and be a no-op returning a round-trippable token otherwise. That is a
 * REIMPLEMENTATION of the observable contract (docs/COMPATIBILITY.md), not a
 * stub: the argument decides the behaviour, and the answer it gives is the one
 * the machine can truthfully give.
 *
 * WHY THE BINDING AND NOT THE KERNEL. GEMDOS 0x20 stays unimplemented in
 * tos-api (EINVFN), exactly as an honest "NewTOS has no supervisor mode to
 * hand out". A trap could not do better -- it cannot return the caller's SP
 * meaningfully without inventing a privilege model -- and EmuTOS itself keeps
 * Super out of the BDOS function table for the same structural reason: it is
 * not a service call, it is a manipulation of the caller's own machine state.
 * The place to resolve caller-side machine state is the caller-side binding.
 *
 * The argument is evaluated exactly once (callers pass expressions).
 */
#define	       Super(ptr)					       \
__extension__								\
({									\
	long __super_arg = (long)(ptr);					\
	long __super_ret;						\
									\
	if (__super_arg == 1L) {					\
		/* x20_inq from user mode (rwa.S:334-339). */		\
		__super_ret = 0L;					\
	} else {							\
		/* No mode to switch; hand back SP_EL0 as the token. */	\
		__asm__ ("mov %0, sp" : "=r"(__super_ret));		\
	}								\
	__super_ret;							\
})

/*
 * SuperToUser -- mintlib's stack-safe way to close the bracket.
 *
 * On m68k it exists only because of a real TOS/EmuTOS bug: if the stack
 * pointer moved between Super(0) and Super(oldssp), the reconstructed USP is
 * wrong (x20_sup's `addq.l #6,a0` assumes an untouched frame), so the binding
 * saved and restored the stack pointer around the trap by hand.
 *
 * On NewTOS there is no mode change, hence no stack reconstruction, hence
 * nothing to compensate: it is Super's closing half with the result discarded.
 * The argument is still evaluated, because callers pass expressions.
 *
 * This is not cosmetic. The m68k inline assembly that used to stand here does
 * not compile for AArch64 ("unknown register name 'a0' in asm"), and that
 * alone cost the libc build two real files -- mintlib/setsysvar.c (the
 * set_sysvar_to_short/long pair) and unix/sysinfo.c.
 */
#define SuperToUser(ptr)						\
	((void)Super(ptr))

#define	       Tgetdate()					       \
       (short)trap_1_w((short)(0x2A))
#define	       Tsetdate(date)					       \
       (long)trap_1_ww((short)(0x2B),(short)(date))
#define	       Tgettime()					       \
       (short)trap_1_w((short)(0x2C))
#define	       Tsettime(time)					       \
       (long)trap_1_ww((short)(0x2D),(short)(time))
#define	       Fgetdta()					       \
       (_DTA *)trap_1_w((short)(0x2F))
#define	       Sversion()					       \
       (short)trap_1_w((short)(0x30))
#define	       Ptermres(save,rv)				       \
       (void)trap_1_wlw((short)(0x31),(long)(save),(short)(rv))
#define	       Dfree(buf,d)					       \
       (long)trap_1_wlw((short)(0x36),(long)(buf),(short)(d))
#define	       Dcreate(path)					       \
       (short)trap_1_wl((short)(0x39),(long)(path))
#define	       Ddelete(path)					       \
       (long)trap_1_wl((short)(0x3A),(long)(path))
#define	       Dsetpath(path)					       \
       (long)trap_1_wl((short)(0x3B),(long)(path))
#define	       Fcreate(fn,mode)					       \
       (long)trap_1_wlw((short)(0x3C),(long)(fn),(short)(mode))
#define	       Fopen(fn,mode)					       \
       (long)trap_1_wlw((short)(0x3D),(long)(fn),(short)(mode))
#define	       Fclose(handle)					       \
       (long)trap_1_ww((short)(0x3E),(short)(handle))
#define	       Fread(handle,cnt,buf)				       \
       (long)trap_1_wwll((short)(0x3F),(short)(handle),	       \
			 (long)(cnt),(long)(buf))
#define	       Fwrite(handle,cnt,buf)				       \
       (long)trap_1_wwll((short)(0x40),(short)(handle),	       \
			 (long)(cnt),(long)(buf))
#define	       Fdelete(fn)					       \
       (long)trap_1_wl((short)(0x41),(long)(fn))
#define	       Fseek(where,handle,how)				       \
       (long)trap_1_wlww((short)(0x42),(long)(where),	       \
			 (short)(handle),(short)(how))
#define	       Fattrib(fn,rwflag,attr)				       \
       (short)trap_1_wlww((short)(0x43),(long)(fn),	       \
			  (short)(rwflag),(short)(attr))
#define	       Fdup(handle)					       \
       (long)trap_1_ww((short)(0x45),(short)(handle))
#define	       Fforce(Hstd,Hnew)				       \
       (long)trap_1_www((short)(0x46),(short)(Hstd),(short)(Hnew))
#define	       Dgetpath(buf,d)					       \
       (long)trap_1_wlw((short)(0x47),(long)(buf),(short)(d))
#define	       Malloc(size)					       \
       (long)trap_1_wl((short)(0x48),(long)(size))
#define	       Mfree(ptr)					       \
       (long)trap_1_wl((short)(0x49),(long)(ptr))
#define	       Mshrink(ptr,size)				       \
       (long)trap_1_wwll((short)(0x4A),(short)0,(long)(ptr),(long)(size))
#define	       Pexec(mode,prog,tail,env)		       \
       (long)trap_1_wwlll((short)(0x4B),(short)(mode),(long)(prog),   \
			   (long)(tail),(long)(env))
#define	       Pterm(rv)					       \
       (void)trap_1_ww((short)(0x4C),(short)(rv))
#define	       Fsfirst(filespec,attr)				       \
       (long)trap_1_wlw((short)(0x4E),(long)(filespec),(short)(attr))
#define	       Fsnext()						       \
       (long)trap_1_w((short)(0x4F))
#define	       Frename(zero,old,_new)				       \
       (short)trap_1_wwll((short)(0x56),(short)(zero),	       \
			  (long)(old),(long)(_new))
#define	       Fdatime(timeptr,handle,rwflag)			       \
       (long)trap_1_wlww((short)(0x57),(long)(timeptr),	       \
			 (short)(handle),(short)(rwflag))
#define	       Flock(handle,mode,start,length)			       \
       (long)trap_1_wwwll((short)(0x5C),(short)(handle),       \
			  (short)(mode),(long)(start),(long)(length))

/*
 *     BIOS    (trap13)
 */
#define Getmpb(ptr)					       \
       (void)trap_13_wl((short)(0x00),(long)(ptr))
#define	       Bconstat(dev)					       \
       (short)trap_13_ww((short)(0x01),(short)(dev))
#define	       Bconin(dev)					       \
       (long)trap_13_ww((short)(0x02),(short)(dev))
#define	       Bconout(dev,c)					       \
       (long)trap_13_www((short)(0x03),(short)(dev),(short)((c) & 0xFF))
/* since AHDI 3.1 there is a new call to Rwabs with one more parameter */
#define	       Rwabs(rwflag,buf,n,sector,d)			\
       (long)trap_13_wwlwww((short)(0x04),(short)(rwflag),(long)(buf), \
			     (short)(n),(short)(sector),(short)(d))
#define	       Lrwabs(rwflag,buf,n,sector,d)			\
       (long)trap_13_wwlwwwl((short)(0x04),(short)(rwflag),(long)(buf),\
			     (short)(n),(short)(-1),(short)(d),(long)sector)
#define	       Setexc(vnum,vptr) 				      \
       (void (*) (void))trap_13_wwl((short)(0x05),(short)(vnum),(long)(vptr))
#define	       Tickcal()					       \
       (long)trap_13_w((short)(0x06))
#define	       Getbpb(d)					       \
       (void *)trap_13_ww((short)(0x07),(short)(d))
#define	       Bcostat(dev)					       \
       (short)trap_13_ww((short)(0x08),(short)(dev))
#define	       Mediach(dev)					       \
       (short)trap_13_ww((short)(0x09),(short)(dev))
#define	       Drvmap()						       \
       (long)trap_13_w((short)(0x0A))
#define	       Kbshift(data)					       \
       (long)trap_13_ww((short)(0x0B),(short)(data))
#define	       Getshift()					       \
	Kbshift(-1)


/*
 *     XBIOS   (trap14)
 */

#define	       Initmous(type,param,vptr)			       \
       (void)trap_14_wwll((short)(0x00),(short)(type),	       \
			  (long)(param),(long)(vptr))
#define Ssbrk(size)					       \
       (void *)trap_14_ww((short)(0x01),(short)(size))
#define	       Physbase()					       \
       (void *)trap_14_w((short)(0x02))
#define	       Logbase()					       \
       (void *)trap_14_w((short)(0x03))
#define	       Getrez()						       \
       (short)trap_14_w((short)(0x04))
#define	       Setscreen(lscrn,pscrn,rez)			       \
       (void)trap_14_wllw((short)(0x05),(long)(lscrn),(long)(pscrn), \
			  (short)(rez))
#define	       Setpalette(palptr)				       \
       (void)trap_14_wl((short)(0x06),(long)(palptr))
#define	       Setcolor(colornum,mixture)			       \
       (short)trap_14_www((short)(0x07),(short)(colornum),(short)(mixture))
#define	       Floprd(buf,x,d,sect,trk,side,n)			       \
       (short)trap_14_wllwwwww((short)(0x08),(long)(buf),(long)(x), \
	 (short)(d),(short)(sect),(short)(trk),(short)(side),(short)(n))
#define	       Flopwr(buf,x,d,sect,trk,side,n)			       \
       (short)trap_14_wllwwwww((short)(0x09),(long)(buf),(long)(x), \
	       (short)(d),(short)(sect),(short)(trk),(short)(side),(short)(n))
#define	       Flopfmt(buf,x,d,spt,t,sd,i,m,v)		       \
       (short)trap_14_wllwwwwwlw((short)(0x0A),(long)(buf),(long)(x), \
	  (short)(d),(short)(spt),(short)(t),(short)(sd),(short)(i),  \
	  (long)(m),(short)(v))
#define	       Midiws(cnt,ptr)					       \
       (void)trap_14_wwl((short)(0x0C),(short)(cnt),(long)(ptr))
#define	       Mfpint(vnum,vptr)				       \
       (void)trap_14_wwl((short)(0x0D),(short)(vnum),(long)(vptr))
#define	       Iorec(ioDEV)					       \
       (void *)trap_14_ww((short)(0x0E),(short)(ioDEV))
#define	       Rsconf(baud,flow,uc,rs,ts,sc)			       \
       (long)trap_14_wwwwwww((short)(0x0F),(short)(baud),(short)(flow), \
			  (short)(uc),(short)(rs),(short)(ts),(short)(sc))
	/* ret old val: MSB -> ucr:8, rsr:8, tsr:8, scr:8 <- LSB */
#define	       Keytbl(nrml,shft,caps)				       \
       (void *)trap_14_wlll((short)(0x10),(long)(nrml), \
			    (long)(shft),(long)(caps))
#define	       Random()						       \
       (long)trap_14_w((short)(0x11))
#define	       Protobt(buf,serial,dsktyp,exec)			       \
       (void)trap_14_wllww((short)(0x12),(long)(buf),(long)(serial), \
			   (short)(dsktyp),(short)(exec))
#define	       Flopver(buf,x,d,sect,trk,sd,n)			       \
       (short)trap_14_wllwwwww((short)(0x13),(long)(buf),(long)(x),(short)(d),\
	       (short)(sect),(short)(trk),(short)(sd),(short)(n))
#define	       Scrdmp()						       \
       (void)trap_14_w((short)(0x14))
#define	       Cursconf(rate,attr)				       \
       (short)trap_14_www((short)(0x15),(short)(rate),(short)(attr))
#define	       Settime(time)					       \
       (void)trap_14_wl((short)(0x16),(long)(time))
#define	       Gettime()					       \
       (long)trap_14_w((short)(0x17))
#define	       Bioskeys()					       \
       (void)trap_14_w((short)(0x18))
#define	       Ikbdws(len_minus1,ptr)				       \
       (void)trap_14_wwl((short)(0x19),(short)(len_minus1),(long)(ptr))
#define	       Jdisint(vnum)					       \
       (void)trap_14_ww((short)(0x1A),(short)(vnum))
#define	       Jenabint(vnum)					       \
       (void)trap_14_ww((short)(0x1B),(short)(vnum))
#define	       Giaccess(data,reg)				       \
       (short)trap_14_www((short)(0x1C),(short)(data),(short)(reg))
#define	       Offgibit(ormask)					       \
       (void)trap_14_ww((short)(0x1D),(short)(ormask))
#define	       Ongibit(andmask)					       \
       (void)trap_14_ww((short)(0x1E),(short)(andmask))
#define	       Xbtimer(timer,ctrl,data,vptr)			       \
       (void)trap_14_wwwwl((short)(0x1F),(short)(timer),(short)(ctrl), \
			   (short)(data),(long)(vptr))
#define	       Dosound(ptr)					       \
       trap_14_wl((short)(0x20),(long)(ptr))
#define	       Setprt(config)					       \
       (short)trap_14_ww((short)(0x21),(short)(config))
#define	       Kbdvbase()					       \
       (_KBDVECS*)trap_14_w((short)(0x22))
#define	       Kbrate(delay,reprate)				       \
       (short)trap_14_www((short)(0x23),(short)(delay),(short)(reprate))
#define	       Prtblk(pblkptr)					       \
       (void)trap_14_wl((short)(0x24),(long)(pblkptr)) /* obsolete ? */
#define	       Vsync()						       \
       (void)trap_14_w((short)(0x25))
/*
 * ---------------------------------------------------------------------------
 * Supexec (XBIOS 0x26) -- NewTOS/AArch64. See TASK/X01 and Super() above.
 * ---------------------------------------------------------------------------
 *
 * EmuTOS' whole implementation is two instructions (bios/vectors.S:1376):
 *
 *	_supexec:
 *		movea.l 4(a7),a0
 *		jmp     (a0)
 *
 * It does NOT raise anything -- the trap has already put the CPU in supervisor
 * mode -- it only jumps to the caller's function, whose `rts` then returns
 * straight to Supexec's caller with the callee's LONG still in d0. So the
 * observable contract is exactly: "call this function, give me its result".
 *
 * On NewTOS the trap would give no privilege to pass on (an EL0 caller stays
 * at EL0), and the callers do not want any: they want low memory, which their
 * own address space already holds (see Super() above). What remains of the
 * contract is the part that still has meaning -- calling the function -- and
 * on AArch64 that is a plain indirect call, the exact analogue of `jmp (a0)`
 * minus a mode the machine cannot grant.
 *
 * The callee's m68k prototype is `long f(void)` in every call site we have
 * (TeraDesk main.c:2118 `rv`, library/utility/nf_ops.c:260 `_nf_detect`,
 * cflib-21/cookie.c:76 `get_cookie_adr`), which is also what the XBIOS
 * contract says, so the cast is to that type. Going through `long` first
 * keeps clang from complaining about the function-pointer cast when the
 * caller's declared type differs in its (unused) parameter list.
 *
 * WARNING for whoever reads this next: making Supexec real also makes
 * TeraDesk's brute-force shutdown real. main.c:2118 calls `Supexec(rv)` with
 * rv == lobo() (main.c:1723), an endless loop whose job is to lobotomise the
 * machine, or with a reset vector read out of the OS header. Before this
 * change XBIOS 0x26 answered 38 and TeraDesk fell through to a clean
 * `appl_exit(); return 0;`. That path is menu-driven ("Quit -> Shutdown") and
 * is not reached by a boot, but it is a genuine behaviour change and it is
 * flagged in TASK/X01.
 */
#define	       Supexec(funcptr)					       \
       ((long)((long (*)(void))(long)(funcptr))())
#define			 Puntaes() \
		 (void)trap_14_w((short)(0x27))
#define	       Floprate(drive,rate)				       \
       (short)trap_14_www((short)(0x29),(short)(drive),(short)(rate))
#define	       Blitmode(flag)					       \
       (short)trap_14_ww((short)(0x40),(short)(flag))
/*
 * Flag:
 *  -1: get config
 * !-1: set config	previous config returned
 *	bit
 *	 0	0 blit mode soft	1 blit mode hardware
 *	 1	0 no blitter		1 blitter present
 *	2..14   reserved
 *	 15	must be zero on set/returned as zero
 * blitmode (bit 0) forced to soft if no blitter(bit 1 == 0).
 */

/*
 * extensions for TT TOS
 */

#define         Mxalloc(amt,flag)					\
	(long)trap_1_wlw((short)(0x44),(long)(amt),(short)(flag))
#define		Maddalt(start,size)					\
	(long)trap_1_wll((short)(0x14),(long)(start),(long)(size))

#define         EsetShift(mode)						\
	(void)trap_14_ww((short)(80),(short)mode)
#define         EgetShift()						\
	(short)trap_14_w((short)(81))
#define         EsetBank(bank)						\
	(short)trap_14_ww((short)(82),(short)bank)
#define         EsetColor(num,val)					\
	(short)trap_14_www((short)(83),(short)num,(short)val)
#define         EsetPalette(start,count,ptr)				\
	(void)trap_14_wwwl((short)(84),(short)start,(short)count,(long)ptr)
#define         EgetPalette(start,count,ptr)				\
	(void)trap_14_wwwl((short)(85),(short)start,(short)count,(long)ptr)
#define         EsetGray(mode)						\
	(short)trap_14_ww((short)(86),(short)mode)
#define         EsetSmear(mode)						\
	(short)trap_14_ww((short)(87),(short)mode)

#define		DMAread(sector,count,buffer,devno)			\
	(long)trap_14_wlwlw((short)0x2a,(long)sector,(short)count,(long)buffer, \
			    (short)devno)
#define		DMAwrite(sector,count,buffer,devno)			\
	(long)trap_14_wlwlw((short)0x2b,(long)sector,(short)count,(long)buffer, \
			(short)devno)
#define		Bconmap(dev)						\
	(long)trap_14_ww((short)0x2c,(short)(dev))
#define		NVMaccess(op,start,count,buf)				\
	(short)trap_14_wwwwl((short)0x2e,(short)op,(short)start,(short)count, \
			(long)buf)

/*  Wake-up call for ST BOOK -- takes date/time pair in DOS format. */

#define	       Waketime(w_date, w_time)					\
       (void)trap_14_www((short)(0x2f),(unsigned short)(w_date),	\
				       (unsigned short)(w_time))

__END_DECLS


#endif /* _MINT_OSBIND_H */
