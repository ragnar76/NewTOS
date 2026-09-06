/*
 * runner_rast.h -- the VDI raster runner (spec sections 4, 5 and 7).
 *
 * Same shape as core/runner.h, and deliberately so: reset the workstation,
 * lay the two pixel buffers, poison every parameter array, lay the case
 * values over the poison, snapshot contrl, issue the call, write the record.
 * Records are buffered in RAM and written once at the end.
 *
 * Two things differ from the plain VDI runner, and both come from the
 * opcodes rather than from the harness:
 *
 *   - contrl is fifteen WORDs, not twelve, because an AArch64 caller packs
 *     the destination MFDB pointer at contrl[11..14] (gemlib gem_vdiP.h:177,
 *     204; VDI_CNTRLMAX = 15, mt_gem.h:3508).  A twelve-WORD contrl would be
 *     overrun by the call itself.
 *   - the two pixel buffers are NOT poisoned.  A copy out of an all-0xDEAD
 *     source proves nothing about where the bits went, so they are laid with
 *     td_rast_fill, which gives every word a different value.  Everything
 *     the call does not touch keeps its fill, which is the same absence
 *     measurement the poison performs elsewhere.
 */
#ifndef TOSDUMP_RUNNER_RAST_H
#define TOSDUMP_RUNNER_RAST_H

#include "cases.h"
#include "gem.h"
#include "runner.h"

int td_run_raster(const struct td_rast_case_table *ct, const struct td_wk *wk,
                  uint8_t *out, size_t out_bytes);

#endif /* TOSDUMP_RUNNER_RAST_H */
