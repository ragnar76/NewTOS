/*
 * runner_aes.h -- poison, lay the case, call, record: the AES half
 * (spec sections 4 and 7).
 *
 * The counterpart of runner.h.  One case is: poison every array, lay the
 * case's control words and int_in values over the poison, issue the call
 * through the ABI backend, write the record.  Records are buffered in RAM
 * and written once at the end (spec section 7), so the runner fills a
 * caller-provided buffer and never touches a file.
 *
 * TWO deliberate differences from the VDI runner, both forced by the AES:
 *
 * 1. THERE IS NO RESET (spec section 5), and that is a decision, not an
 *    omission.  The VDI reset restores the attribute defaults init_wk
 *    establishes, which docs/api/vdi/_state.md documents as a defined set.
 *    The AES has no equivalent: docs/api/aes/_state.md describes process
 *    descriptors, message queues and run lists, and there is no documented
 *    default set to restore.  Round 1 is therefore restricted to opcodes
 *    that do not mutate state another case reads -- see
 *    cases/COVERAGE-AES.md for which opcodes that ruled out and why.  A
 *    real reset lands with round 2, together with the prelude cases that
 *    need it.
 *
 * 2. THERE IS NO "BEFORE" SNAPSHOT.  The VDI record carries contrl[] both
 *    before and after the call (spec section 3, offset 1568); the 120 byte
 *    AES record has no such field, so there is nowhere to put one.  It
 *    needs none: the AES dispatcher copies control[] onto a local buffer
 *    and never writes it back (EmuTOS aes/gemsuper.c:435, and no
 *    counterpart to the intout copy-back at :444-447), so the recorded
 *    control[] after the call IS the "before" state -- and an
 *    implementation that wrote a control slot back shows up as a
 *    difference against the values the case laid.
 *
 * global[] is NOT poisoned and NOT laid by a case.  It is live AES state:
 * crysbind fills it at APPL_INIT (gemsuper.c:92-98) and rs_load / rs_free /
 * rs_gaddr / rs_saddr read it back (gemsuper.c:363-373), so the runner
 * hands the call the application's own live AESGLOBAL -- the one
 * td_gem_open's appl_init filled -- exactly as a real GEM application
 * would, and copies it into the record after every call.
 */
#ifndef TOSDUMP_RUNNER_AES_H
#define TOSDUMP_RUNNER_AES_H

#include "cases.h"

#define TD_RUN_AES_ERR_BUFFER (-1)   /* the buffer cannot hold every record */

/*
 * `global` is the application's live AESGLOBAL array (TD_AES_GLOBAL_N
 * words), normally td_gem_global().  It is passed to every call and
 * recorded after every call; the runner never clears or poisons it.
 */
int td_run_aes(const struct td_aes_case_table *ct, int16_t *global,
               uint8_t *out, size_t out_bytes);

#endif /* TOSDUMP_RUNNER_AES_H */
