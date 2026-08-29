/*
 * runner.h -- reset, poison, call, record (spec sections 4, 5 and 7).
 *
 * One case is: reset the workstation, poison every array, lay the case values
 * over the poison, snapshot contrl, issue the call through the ABI backend,
 * write the record.  Records are buffered in RAM and written once at the end
 * (spec section 7), so the runner fills a caller-provided buffer and never
 * touches a file.
 */
#ifndef TOSDUMP_RUNNER_H
#define TOSDUMP_RUNNER_H

#include "cases.h"
#include "gem.h"

#define TD_RUN_ERR_BUFFER (-1)   /* the buffer cannot hold every record */

/* Applies v_clrwk and re-applies the default attributes.  Exposed because the
 * dumper also runs it once before the first case. */
void td_reset_vdi(const struct td_wk *wk);

int td_run_vdi(const struct td_case_table *ct, const struct td_wk *wk,
               uint8_t *out, size_t out_bytes);

#endif /* TOSDUMP_RUNNER_H */
