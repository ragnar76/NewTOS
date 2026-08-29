/*
 * tosdump.h -- record format for the TOS reference dumper.
 *
 * Spec: docs/TOS-REFERENCE-DUMPER.md, section 3 (record format) and 4 (poison).
 *
 * This header is compiled by three toolchains: the host cc, m68k-atari-mint-gcc
 * (TOS 3.06 target) and clang/aarch64 (NewTOS target).  Per spec section 2.1 it
 * therefore uses fixed-width types only -- never int, short or long -- and the
 * record is written field by field at explicit offsets, never by dumping a
 * struct, so that padding and alignment rules cannot differ between targets.
 */
#ifndef TOSDUMP_H
#define TOSDUMP_H

#include <stdint.h>
#include <stddef.h>

/* ------------------------------------------------------------------ */
/* File header -- 32 bytes (spec section 3)                            */
/* ------------------------------------------------------------------ */

#define TD_MAGIC            "TDMP"
#define TD_MAGIC_LEN        4
#define TD_FORMAT_VERSION   2
#define TD_HEADER_SIZE      32

#define TD_HDR_OFF_MAGIC        0   /* 4  bytes, "TDMP"                   */
#define TD_HDR_OFF_VERSION      4   /* u16, format version                */
#define TD_HDR_OFF_AREA         6   /* u16, TD_AREA_*                     */
#define TD_HDR_OFF_RECSIZE      8   /* u16, record size in bytes          */
#define TD_HDR_OFF_RECCOUNT    10   /* u16, record count                  */
#define TD_HDR_OFF_BUILDID     12   /* 4  bytes, git short hash ASCII     */
#define TD_HDR_OFF_CASECRC     16   /* u32, CRC32 of the case table       */
#define TD_HDR_OFF_RESERVED    20   /* 12 bytes, must be zero             */
#define TD_HDR_BUILDID_LEN      4
#define TD_HDR_RESERVED_LEN    12

/* area codes */
#define TD_AREA_BIOS        1
#define TD_AREA_XBIOS       2
#define TD_AREA_GEMDOS      3
#define TD_AREA_VDI         4
#define TD_AREA_AES         5

/* ------------------------------------------------------------------ */
/* Case tables (spec section 6)                                        */
/* ------------------------------------------------------------------ */

/* Upper bound on cases per area.  Fixed so that the target program needs no
 * allocator: it runs on a TT with TOS 3.06 and on NewTOS alike. */
#define TD_MAX_CASES        256
#define TD_COMMENT_LEN       64

/* retclass column (spec section 6), BIOS / XBIOS / GEMDOS only */
#define TD_RET_EXACT        0
#define TD_RET_IGNORE       1
#define TD_RET_NONZERO      2
#define TD_RET_HANDLE       3
#define TD_RET_PTR_RAM      4

/*
 * The trap's own return value.
 *
 * The spec's record layout (section 3) leaves four bytes reserved after the
 * opcode and has no field for what the trap returns.  The VDI trap does return
 * something: d0 carries flip_y (docs/api/vdi/_dispatcher.md, "Return d0"), and
 * docs/api/vdi/_working-list.md D-080 records that vsm_height, vst_height,
 * vst_point and vst_font set it as a side effect.  That is an observable
 * output of a call, so it is measured like every other output, in the space
 * the layout already reserved.  Format version 2.
 */

/* ------------------------------------------------------------------ */
/* Poison (spec section 4)                                             */
/* ------------------------------------------------------------------ */

#define TD_POISON16         ((int16_t)0xDEAD)
#define TD_POISON_HI        ((uint8_t)0xDE)
#define TD_POISON_LO        ((uint8_t)0xAD)

/* ------------------------------------------------------------------ */
/* VDI record -- 1592 bytes                                            */
/* ------------------------------------------------------------------ */

#define TD_VDI_CONTRL_N     12
#define TD_VDI_INTIN_N     128
#define TD_VDI_PTSIN_N     256
#define TD_VDI_INTOUT_N    128
#define TD_VDI_PTSOUT_N    256

#define TD_VDI_RECORD_SIZE      1592
#define TD_VDI_OFF_CASEID          0   /* u16                             */
#define TD_VDI_OFF_OPCODE          2   /* u16                             */
#define TD_VDI_OFF_RET             4   /* i32, the trap's d0 return       */
#define TD_VDI_OFF_CONTRL          8   /* contrl[12] after the call       */
#define TD_VDI_OFF_INTIN          32   /* intin[128]  after the call      */
#define TD_VDI_OFF_PTSIN         288   /* ptsin[256]  after the call      */
#define TD_VDI_OFF_INTOUT        800   /* intout[128] after the call      */
#define TD_VDI_OFF_PTSOUT       1056   /* ptsout[256] after the call      */
#define TD_VDI_OFF_CONTRL_BEFORE 1568  /* contrl[12] before the call      */

/* ------------------------------------------------------------------ */
/* AES record -- 120 bytes                                             */
/* ------------------------------------------------------------------ */

#define TD_AES_CONTRL_N      5
#define TD_AES_GLOBAL_N     15
#define TD_AES_INTIN_N      16
#define TD_AES_INTOUT_N      7
#define TD_AES_ADDRIN_N      3
#define TD_AES_ADDROUT_N     1

#define TD_AES_RECORD_SIZE   120
#define TD_AES_OFF_CASEID      0   /* u16                                 */
#define TD_AES_OFF_OPCODE      2   /* u16                                 */
#define TD_AES_OFF_CONTRL      4   /* contrl[5]   after                   */
#define TD_AES_OFF_GLOBAL     14   /* global[15]  after                   */
#define TD_AES_OFF_INTIN      44   /* int_in[16]  after                   */
#define TD_AES_OFF_INTOUT     76   /* int_out[7]  after                   */
#define TD_AES_OFF_ADDRIN     90   /* addr_in[3]  after, 32-bit each      */
#define TD_AES_OFF_ADDROUT   102   /* addr_out[1] after, 32-bit           */
#define TD_AES_OFF_RET       106   /* i32, the trap's d0 return           */
#define TD_AES_OFF_RESERVED  110   /* 10 bytes, zero                      */
#define TD_AES_RESERVED_LEN   10

/* ------------------------------------------------------------------ */
/* BIOS / XBIOS / GEMDOS record -- 320 bytes                           */
/* ------------------------------------------------------------------ */

#define TD_CALL_ARGS_N         8
#define TD_CALL_CAPTURE_LEN  256

#define TD_CALL_RECORD_SIZE  320
#define TD_CALL_OFF_CASEID     0   /* u16                                 */
#define TD_CALL_OFF_OPCODE     2   /* u16                                 */
#define TD_CALL_OFF_ARGS       4   /* 8 longs as passed                   */
#define TD_CALL_OFF_RET       36   /* i32, return value (d0)              */
#define TD_CALL_OFF_CAPTURE   40   /* capture buffer                      */
#define TD_CALL_OFF_RESERVED 296   /* 24 bytes, zero                      */
#define TD_CALL_RESERVED_LEN  24

#endif /* TOSDUMP_H */
