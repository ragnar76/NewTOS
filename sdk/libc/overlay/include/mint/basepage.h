/*
 *	BASEPAGE.H	Definition of the basepage structure
 *
 * NewTOS overlay (LP64 ABI correction). Every "pointer or LONG" field of the
 * basepage is a FIXED 32-bit type here, NOT a native `char *` / `long`.
 *
 * On m68k/ILP32 a pointer and a `long` are both 4 bytes, so the classic Atari
 * basepage is 256 bytes with p_env @ 0x2C (44). On AArch64/LP64 a native
 * `char *` / `long` is 8 bytes, which would balloon sizeof(BASEPAGE) to 376 and
 * push p_env to offset 88 -- but the NewTOS kernel writes the classic 256-byte
 * layout (p_env @ 44) via tos-api/src/bdos/process/loader.rs `populate_basepage`
 * (kernel struct tos-types/src/basepage.rs). Using __UINT32_TYPE__ /
 * __INT32_TYPE__ (exactly 32 bits on BOTH ABIs, like the _DTA fix in
 * ostruct.h) keeps this struct byte-for-byte identical to the kernel on every
 * ABI: on ILP32 the widths are unchanged (already 4 bytes), on LP64 they no
 * longer widen to 8. NewTOS EL0 VAs are all < 2^32, so the 32-bit address
 * fields hold them.
 *
 * Consequence: reading an address field into a real pointer is now an
 * int->pointer conversion (a default ERROR under this clang, -Wint-conversion),
 * so the use site needs an explicit `(T *)(uintptr_t)` cast -- see the TeraDesk
 * environm.c overlay (sdk/overlays/teradesk/environm.c). The _Static_assert
 * block pins the ABI-critical offsets on every ABI.
 */

#ifndef _MINT_BASEPAGE_H
# define _MINT_BASEPAGE_H 1

#ifndef	_FEATURES_H
# include <features.h>
#endif

__BEGIN_DECLS

typedef struct basep {
    __UINT32_TYPE__	p_lowtpa;	/* pointer to self (bottom of TPA) */
    __UINT32_TYPE__	p_hitpa;	/* pointer to top of TPA + 1 */
    __UINT32_TYPE__	p_tbase;	/* base of text segment */
    __INT32_TYPE__	p_tlen;		/* length of text segment */
    __UINT32_TYPE__	p_dbase;	/* base of data segment */
    __INT32_TYPE__	p_dlen;		/* length of data segment */
    __UINT32_TYPE__	p_bbase;	/* base of BSS segment */
    __INT32_TYPE__	p_blen;		/* length of BSS segment */
    __UINT32_TYPE__	p_dta;		/* (UNOFFICIAL, DON'T USE) */
    __UINT32_TYPE__	p_parent;	/* pointer to parent's basepage */
    __UINT32_TYPE__	p_reserved;	/* reserved for future use */
    __UINT32_TYPE__	p_env;		/* pointer to environment string */
    char	p_junk[8];
    __INT32_TYPE__	p_undef[18];	/* scratch area... don't touch */
    char	p_cmdlin[128];	/* command line image */
} BASEPAGE;

extern BASEPAGE *_base;

/* Pin the ABI-critical offsets + total size on BOTH ILP32 and LP64. A future
 * regression that widens a field back to a native pointer/long fails here. */
_Static_assert(__builtin_offsetof(BASEPAGE, p_hitpa) == 4,    "p_hitpa@4");
_Static_assert(__builtin_offsetof(BASEPAGE, p_tbase) == 8,    "p_tbase@8");
_Static_assert(__builtin_offsetof(BASEPAGE, p_env)   == 0x2c, "p_env@0x2c(44)");
_Static_assert(sizeof(BASEPAGE) == 256,                       "sizeof(BASEPAGE) == 256");

__END_DECLS

#endif /* _MINT_BASEPAGE_H */
