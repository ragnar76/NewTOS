/*
 * dump.h -- write the buffered records out as one file (spec section 7).
 *
 * "Each writes exactly one output file ... Buffer records in RAM and write
 * once at the end.  Do not write per record."
 */
#ifndef TOSDUMP_DUMP_H
#define TOSDUMP_DUMP_H

#include "record.h"

#define TD_DUMP_ERR_OPEN  (-1)
#define TD_DUMP_ERR_WRITE (-2)
#define TD_DUMP_ERR_SIZE  (-3)

int td_dump_write(const char *path, uint16_t area, uint16_t count,
                  const char *build_id, uint32_t case_crc,
                  const uint8_t *records, size_t bytes);

#endif /* TOSDUMP_DUMP_H */
