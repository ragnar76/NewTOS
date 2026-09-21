#ifndef _GEM_AES_P_
# define _GEM_AES_P_

# ifndef _GEMLIB_H_
#  include "mt_gem.h"
# endif

#ifndef NULL
#  define NULL ((void *)0)
#endif


#ifndef NO_CONST
#  ifdef __GNUC__
#	 define NO_CONST(p) __extension__({ union { const void *cs; void *s; } _x; _x.cs = p; _x.s; })
#  else
#	 define NO_CONST(p) ((void *)(p))
#  endif
#endif


#if defined(__NEWTOS__)

/*
 * NewTOS/AArch64: the m68k inline-asm trap below cannot be expressed on
 * AArch64, so route AES_TRAP through the retargeted C primitive aes() in
 * _gc_aes.c (svc #2, selector 200 in x0, &AESPB in x1 -- see that file for the
 * verified ABI). Every a_*.c binding that uses AES_TRAP() links to it
 * unchanged.
 */
#define AES_TRAP(aespb) aes(&aespb)

#elif defined(__GNUC__) && !defined(__NO_INLINE__)

static inline void
_aes_trap (AESPB * aespb)
{
	__asm__ volatile (
		"move.l	%0,%%d1\n\t"	/* &aespb */
		"move.w	#200,%%d0\n\t"
		"trap	#2"
		:
		: "g"(aespb)
		: "d0","d1","d2","a0","a1","a2","memory","cc"
	);
}
#define AES_TRAP(aespb) _aes_trap(&aespb)

#else

#if defined(__VBCC__)

__regsused("d0/d1/a0/a1") void _aes_trap(__reg("d1")AESPB *) =
  "\tmove.l\td2,-(sp)\n"
  "\tmove.l\ta2,-(sp)\n"
  "\tmove.w\t#200,d0\n"
  "\ttrap\t#2\n"
  "\tmove.l\t(sp)+,a2\n"
  "\tmove.l\t(sp)+,d2";
#define AES_TRAP(aespb) _aes_trap(&aespb)

#else /* no usage of inlines, go the old way */

#define AES_TRAP(aespb) aes(&aespb)

#endif /* __VBCC__ */
#endif


#define AES_PARAMS(opcode,num_intin,num_intout,num_addrin,num_addrout) \
	static short    aes_control[AES_CTRLMAX]={opcode,num_intin,num_intout,num_addrin,num_addrout}; \
	short			aes_intin[AES_INTINMAX];			  \
	short			aes_intout[AES_INTOUTMAX];			  \
	long			aes_addrin[AES_ADDRINMAX];			  \
	long			aes_addrout[AES_ADDROUTMAX];		  \
 														  \
	AESPB aes_params;									  \
  	aes_params.control = &aes_control[0];				  \
  	aes_params.global  = &global_aes[0];				  \
  	aes_params.intin   = &aes_intin[0]; 				  \
  	aes_params.intout  = &aes_intout[0];				  \
  	aes_params.addrin  = &aes_addrin[0];				  \
  	aes_params.addrout = &aes_addrout[0]


#if defined(__NEWTOS__)

/*
 * NewTOS/AArch64 (TASK/T04): a LONG inside the AES int_in/int_out WORD
 * arrays is 32 bits over two WORDs, low word first, stored and loaded in
 * place at &array[n] (docs/COMPATIBILITY.md, "A LONG in int_in/int_out/
 * ptsin/ptsout is 32 bits over two WORDs"). That is the 68k access EmuTOS
 * performs: `*(LONG *)&int_out[1]` (aes/gemwmlib.c:1564, wind_get WF_SCREEN)
 * and `MAKE_ULONG(int_in[1], int_in[0])` (aes/gemsuper.c:143, evnt_timer;
 * include/aesdefs.h:190-191, include/portab.h:139; LONG is 32 bits,
 * include/portab.h:123) leave the same two WORDs on a little-endian machine,
 * and NewTOS's AES reads and writes them that way (tos-api/src/aes/event.rs
 * evnt_timer, aes/mod.rs Aespb::set_intout_long).
 *
 * The pristine accessor dereferences a `long *`. On LP64 that is an EIGHT-
 * byte access at a two-byte-aligned slot: a store spills the upper half over
 * the two WORDs after the LONG, and a load takes the two WORDs after the LONG
 * as its upper half (x_fnts_*, x_edit_evnt, pdlg_get_setsize read four
 * WORDs where the AES wrote two). A 32-bit type with the slot's own
 * alignment (2) and `may_alias` (the slot is declared `short`) makes every
 * accessor exactly the 68k access: one 32-bit `str w`/`ldr w` at &array[n],
 * sign-extended on the way out like a 68k `long` widened to LP64.
 *
 * The pointer accessors below are NOT changed: a pointer packed into a WORD
 * array occupies the caller's own pointer width, N_PTRINTS WORDs (A81; the
 * kernel's `Aespb::intin_ptr` reads it at that width). N_PTRINTS is
 * gem_vdiP.h's constant, repeated here for the AES bindings that declare a
 * packed pointer's WORD count (a_wind_get.c, a_wind_sget.c).
 */
#ifndef N_PTRINTS
#define N_PTRINTS (sizeof(void *) / sizeof(short))
#endif

#ifndef _GEM_WIRE_LONG_T_
#define _GEM_WIRE_LONG_T_
typedef int __attribute__((aligned(2), may_alias)) gem_wire_long;
_Static_assert(sizeof(gem_wire_long) == 4, "gem_wire_long must be the 32-bit 68k LONG");
#endif

static __inline gem_wire_long *__aes_intout_long(short n, short *aes_intout)
{
	return ((gem_wire_long *)(aes_intout   +n));
}
#define aes_intout_long(n)  *__aes_intout_long(n, aes_intout)

static __inline gem_wire_long *__aes_intin_long(short n, short *aes_intin)
{
	return ((gem_wire_long *)(aes_intin   +n));
}
#define aes_intin_long(n)  *__aes_intin_long(n, aes_intin)

static __inline void **__aes_intout_ptr(short n, short *aes_intout)
{
	return ((void **)(aes_intout   +n));
}
#define aes_intout_ptr(n, t)  *((t *)__aes_intout_ptr(n, aes_intout))

static __inline void **__aes_intin_ptr(short n, short *aes_intin)
{
	return ((void **)(aes_intin   +n));
}
#define aes_intin_ptr(n, t)  *((t *)__aes_intin_ptr(n, aes_intin))

#elif defined(__GNUC__)

/* to avoid "dereferencing type-punned pointer" */
static __inline long *__aes_intout_long(short n, short *aes_intout)
{
	return ((long *)(aes_intout   +n));
}
#define aes_intout_long(n)  *__aes_intout_long(n, aes_intout)

static __inline long *__aes_intin_long(short n, short *aes_intin)
{
	return ((long *)(aes_intin   +n));
}
#define aes_intin_long(n)  *__aes_intin_long(n, aes_intin)

static __inline void **__aes_intout_ptr(short n, short *aes_intout)
{
	return ((void **)(aes_intout   +n));
}
#define aes_intout_ptr(n, t)  *((t *)__aes_intout_ptr(n, aes_intout))

static __inline void **__aes_intin_ptr(short n, short *aes_intin)
{
	return ((void **)(aes_intin   +n));
}
#define aes_intin_ptr(n, t)  *((t *)__aes_intin_ptr(n, aes_intin))

#else

#define aes_intout_long(n)  *((long *)(aes_intout+(n)))
#define aes_intin_long(n)  *((long *)(aes_intin+(n)))
#define aes_intout_ptr(n, t)  *((t *)((void **)(aes_intout+(n))))
#define aes_intin_ptr(n, t)  *((t *)((void **)(aes_intin+(n))))

#endif


/* special feature for AES bindings: pointer in parameters (for return values)
 * could be NULL (nice idea by Martin Elsasser against dummy variables) 
 */
#define CHECK_NULLPTR 1

#endif /* _GEM_AES_P_ */
