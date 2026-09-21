#ifndef _GEM_VDI_P_
# define _GEM_VDI_P_

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
 * NewTOS/AArch64: the m68k inline-asm traps below pack the CONTRL words with a
 * big-endian `move.l` sequence that would be an endian bug on little-endian
 * AArch64. Instead fill CONTRL element-by-element in portable C (endian-safe)
 * and route through the retargeted C primitive vdi() in _gc_vdi.c (svc #2,
 * selector 115 in x0, &VDIPB in x1 -- see that file for the verified ABI).
 * Every v_*.c binding that uses the VDI_TRAP family links to it unchanged.
 */
#define VDI_TRAP_ESC(vdipb, handle, opcode, subop, cntrl_1, cntrl_3) \
	vdipb.control[0] = opcode;  \
	vdipb.control[1] = cntrl_1; \
	vdipb.control[3] = cntrl_3; \
	vdipb.control[5] = subop;   \
	vdipb.control[6] = handle;  \
	vdi (&vdipb);

#define VDI_TRAP_00(vdipb, handle, opcode) \
	VDI_TRAP_ESC (vdipb, handle, opcode, 0, 0, 0)

#elif defined(__GNUC__) && !defined(__NO_INLINE__)

static inline void
_vdi_trap_esc (VDIPB * vdipb,
               long cntrl_0_1, long cntrl_3, long cntrl_5, short handle)
{
	__asm__ volatile (
		"movea.l	%0,%%a0\n\t"	/* &vdipb */
		"move.l	%%a0,%%d1\n\t"
		"move.l	(%%a0),%%a0\n\t"	/* vdipb->control */
		"move.l	%1,(%%a0)+\n\t"	/* cntrl_0, cntrl_1 */
		"move.l	%2,(%%a0)+\n\t"	/* cntrl_2, cntrl_3 */
		"move.l	%3,(%%a0)+\n\t"	/* cntrl_4, cntrl_5 */
		"move.w	%4,(%%a0)\n\t"	/* handle */
		"move.w	#115,%%d0\n\t"	/* 0x0073 */
		"trap	#2"
		:
		: "g"(vdipb), "g"(cntrl_0_1), "g"(cntrl_3), "g"(cntrl_5), "g"(handle)
		: "d0", "d1", "d2", "a0", "a1", "a2", "memory", "cc"
	);
}
#define VDI_TRAP_ESC(vdipb, handle, opcode, subop, cntrl_1, cntrl_3) \
	_vdi_trap_esc (&vdipb, (opcode##uL<<16)|cntrl_1, cntrl_3, subop, handle)

static inline void
_vdi_trap_00 (VDIPB * vdipb, long cntrl_0_1, short handle)
{
	__asm__ volatile (
		"movea.l %0,%%a0\n\t"	/* &vdipb */
		"move.l  %%a0,%%d1\n\t"
		"move.l  (%%a0),%%a0\n\t"	/* vdipb->control */
		"move.l  %1,(%%a0)+\n\t"	/* cntrl_0, cntrl_1 */
		"moveq   #0,%%d0\n\t"
		"move.l  %%d0,(%%a0)+\n\t"	/* cntrl_2, cntrl_3 */
		"move.l  %%d0,(%%a0)+\n\t"	/* cntrl_4, cntrl_5 */
		"move.w  %2,(%%a0)\n\t"	/* handle */
		"move.w  #115,%%d0\n\t"	/* 0x0073 */
		"trap    #2"
		:
		: "g"(vdipb), "g"(cntrl_0_1), "g"(handle)
		: "d0", "d1", "d2", "a0", "a1", "a2", "memory", "cc"
	);
}
#define VDI_TRAP_00(vdipb, handle, opcode) \
	_vdi_trap_00 (&vdipb, (opcode##uL<<16), handle)


#else 

#if defined(__VBCC__)

__regsused("d0/d1/a0/a1") void _vdi_trap_esc(
        __reg("a0")VDIPB *,__reg("d0")long,__reg("d1")long,
        __reg("d2")long,__reg("d3")short) =
  "\tmove.l\td2,-(sp)\n"
  "\tmove.l\ta2,-(sp)\n"
  "\tmove.l\t(a0),a1\n"
  "\tmovem.l\td0-d2,(a1)\n"
  "\tmove.w\td3,12(a1)\n"
  "\tmove.l\ta0,d1\n"
  "\tmoveq\t#115,d0\n"
  "\ttrap\t#2\n"
  "\tmove.l\t(sp)+,a2\n"
  "\tmove.l\t(sp)+,d2";
  
#define VDI_TRAP_ESC(vdipb, handle, opcode, subop, cntrl_1, cntrl_3) \
        _vdi_trap_esc (&vdipb, (opcode##uL<<16)|cntrl_1, cntrl_3, subop, handle)

__regsused("d0/d1/a0/a1") void _vdi_trap_00(
        __reg("a0")VDIPB *,__reg("d0")long,__reg("d1")short) =
  "\tmove.l\td2,-(sp)\n"
  "\tmove.l\ta2,-(sp)\n"
  "\tmove.l\t(a0),a1\n"
  "\tmove.l\td0,(a1)+\n"
  "\tmoveq\t#0,d0\n"
  "\tmove.l\td0,(a1)+\n"
  "\tmove.l\td0,(a1)+\n"
  "\tmove.w\td1,(a1)\n"
  "\tmove.l\ta0,d1\n"
  "\tmoveq\t#115,d0\n"
  "\ttrap\t#2\n"
  "\tmove.l\t(sp)+,a2\n"
  "\tmove.l\t(sp)+,d2";  
  
#define VDI_TRAP_00(vdipb, handle, opcode) \
	_vdi_trap_00 (&vdipb, (opcode##uL<<16), handle)
 
#else /* no usage of inlines, go the old way */

#define VDI_TRAP_ESC(vdipb, handle, opcode, subop, cntrl_1, cntrl_3) \
	vdipb.control[0] = opcode;  \
	vdipb.control[1] = cntrl_1; \
	vdipb.control[3] = cntrl_3; \
	vdipb.control[5] = subop;   \
	vdipb.control[6] = handle;  \
	vdi (&vdipb);

#define VDI_TRAP_00(vdipb, handle, opcode) \
	VDI_TRAP_ESC (vdipb, handle, opcode, 0, 0, 0)
#endif /* __VBCC__ */
#endif


#define VDI_TRAP(vdipb, handle, opcode, cntrl_1, cntrl_3) \
	VDI_TRAP_ESC(vdipb, handle, opcode, 0, cntrl_1, cntrl_3)

#define VDI_PARAMS(a,b,c,d,e) \
	VDIPB vdi_params;         \
	vdi_params.control = a;   \
	vdi_params.intin   = b;   \
	vdi_params.ptsin   = c;   \
	vdi_params.intout  = d;   \
	vdi_params.ptsout  = e;


/* special feature for VDI bindings: pointer in parameters (for return values)
 * could be NULL (nice idea by Martin Elsasser against dummy variables) 
 */
#define CHECK_NULLPTR 1

/* special feature for VDI bindings: set VDIPB::intout and VDIPB::ptsout to
 * vdi_dummy array intead of NULL against crashes if some crazy VDI drivers
 * tries to write something in ptsout/intout.
 */ 
#define USE_VDI_DUMMY 1

#if USE_VDI_DUMMY
	/* use dummy array vdi_dummy[] from vdi_dummy.c */
	extern short vdi_dummy[];
#else
	/* replace vdi_dummy in VDIPB by NULL pointer */
	#define vdi_dummy 0L
#endif


#define N_PTRINTS (sizeof(void *) / sizeof(short))

#if defined(__NEWTOS__)

/*
 * NewTOS/AArch64 (TASK/T04): a LONG inside a VDI/AES WORD array is 32 bits
 * over two WORDs, low word first, stored and loaded in place at &array[n]
 * (docs/COMPATIBILITY.md, "A LONG in int_in/int_out/ptsin/ptsout is 32 bits
 * over two WORDs"). That is the 68k access EmuTOS performs: `*(LONG *)
 * &int_out[1]` (aes/gemwmlib.c:1564) and `MAKE_ULONG(int_in[1], int_in[0])`
 * (aes/gemsuper.c:143, include/aesdefs.h:190-191, include/portab.h:139; LONG
 * is 32 bits, include/portab.h:123) leave the same two WORDs on a
 * little-endian machine.
 *
 * The pristine accessor dereferences a `long *`. On LP64 that is an EIGHT-
 * byte access at a two-byte-aligned slot: a store spills the upper half over
 * the two WORDs after the LONG -- or past the end of an exactly sized local
 * array, where it hit the binding's own control[0] (v_create_ctab,
 * vst_track_offset, vst_arbpt32 reached the VDI with opcode 0) or the
 * caller's frame (v_value2color) -- and a load takes the two WORDs after the
 * LONG as its upper half (every *_intout_long reader, ten of them off the
 * end of the array). A 32-bit type with the slot's own alignment (2) and
 * `may_alias` (the slot is declared `short`) makes every accessor exactly
 * the 68k access: one 32-bit `str w`/`ldr w` at &array[n], sign-extended on
 * the way out like a 68k `long` widened to LP64.
 *
 * The pointer accessors below are NOT changed: a pointer in a WORD array
 * occupies the caller's own pointer width, N_PTRINTS WORDs (A81; the kernel's
 * `Aespb::intin_ptr` / `Vdipb::contrl_ptr` read it at that width).
 */
#ifndef _GEM_WIRE_LONG_T_
#define _GEM_WIRE_LONG_T_
typedef int __attribute__((aligned(2), may_alias)) gem_wire_long;
_Static_assert(sizeof(gem_wire_long) == 4, "gem_wire_long must be the 32-bit 68k LONG");
#endif

static __inline gem_wire_long *__vdi_array_long(short n, short *array)
{
	return ((gem_wire_long *)(array   +n));
}

static __inline void **__vdi_array_ptr(short n, short *array)
{
	return ((void**)(array + n));
}

#elif defined(__GNUC__)

/* to avoid "dereferencing type-punned pointer" */
static __inline long *__vdi_array_long(short n, short *array)
{
	return ((long *)(array   +n));
}

static __inline void **__vdi_array_ptr(short n, short *array)
{
	return ((void**)(array + n));
}

#else

#define __vdi_array_ptr(n, array)   ((void **)(array + (n)))
#define __vdi_array_long(n, array)   ((long *)(array + (n)))

#endif

#define vdi_intin_long(n)  *__vdi_array_long(n, vdi_intin)
#define vdi_intout_long(n)  *__vdi_array_long(n, vdi_intout)
#define vdi_ptsin_long(n)  *__vdi_array_long(n, vdi_ptsin)
#define vdi_ptsout_long(n)  *__vdi_array_long(n, vdi_ptsout)

#define vdi_control_ptr(n, t)  *((t *)__vdi_array_ptr(7 + (n) * N_PTRINTS, vdi_control))
#define vdi_intin_ptr(n, t)  *((t *)__vdi_array_ptr(n, vdi_intin))
#define vdi_intout_ptr(n, t)  *((t *)__vdi_array_ptr(n, vdi_intout))
#define vdi_ptsin_ptr(n, t)  *((t *)__vdi_array_ptr(n, vdi_ptsin))
#define vdi_ptsout_ptr(n, t)  *((t *)__vdi_array_ptr(n, vdi_ptsout))

# endif /* _GEM_VDI_P_ */
