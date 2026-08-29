/*
 * record.h -- big-endian record writer / reader for the TOS reference dumper.
 *
 * Spec: docs/TOS-REFERENCE-DUMPER.md sections 2.1 and 3.
 *
 * Every field of every record goes through td_store_be16 / td_store_be32.
 * Reserved fields and the header are routed through them too, exactly as the
 * spec demands, so that a byte-order bug cannot hide in a field nobody looks at.
 */
#ifndef TOSDUMP_RECORD_H
#define TOSDUMP_RECORD_H

#include "tosdump.h"

/* ---- byte order (spec section 2.1) ------------------------------------ */

void     td_store_be16(uint8_t *p, uint16_t v);
void     td_store_be32(uint8_t *p, uint32_t v);
uint16_t td_load_be16(const uint8_t *p);
uint32_t td_load_be32(const uint8_t *p);

/* ---- CRC32 (case table identity) -------------------------------------- */

uint32_t td_crc32(const uint8_t *data, size_t len);

/* ---- file header ------------------------------------------------------ */

struct td_header {
	uint16_t version;
	uint16_t area;
	uint16_t record_size;
	uint16_t record_count;
	char     build_id[TD_HDR_BUILDID_LEN + 1];  /* NUL-terminated copy */
	uint32_t case_crc;
};

void td_header_write(uint8_t *hdr, const struct td_header *h);

/* Returns 0 on success, negative on a malformed header. */
#define TD_ERR_MAGIC    (-1)
#define TD_ERR_VERSION  (-2)
#define TD_ERR_RESERVED (-3)
#define TD_ERR_RECSIZE  (-4)
#define TD_ERR_AREA     (-5)
#define TD_ERR_SIZE     (-6)
int td_header_parse(const uint8_t *hdr, size_t len, struct td_header *out);

/* Record size mandated for an area, or 0 if the area code is unknown. */
uint16_t td_area_record_size(uint16_t area);
const char *td_area_name(uint16_t area);

/* ---- per-area frames --------------------------------------------------- */

struct td_vdi_arrays {
	int16_t contrl[TD_VDI_CONTRL_N];
	int16_t intin[TD_VDI_INTIN_N];
	int16_t ptsin[TD_VDI_PTSIN_N];
	int16_t intout[TD_VDI_INTOUT_N];
	int16_t ptsout[TD_VDI_PTSOUT_N];
};

struct td_aes_arrays {
	int16_t  contrl[TD_AES_CONTRL_N];
	int16_t  global[TD_AES_GLOBAL_N];
	int16_t  int_in[TD_AES_INTIN_N];
	int16_t  int_out[TD_AES_INTOUT_N];
	uint32_t addr_in[TD_AES_ADDRIN_N];
	uint32_t addr_out[TD_AES_ADDROUT_N];
};

struct td_call_frame {
	uint32_t args[TD_CALL_ARGS_N];
	int32_t  ret;
	uint8_t  capture[TD_CALL_CAPTURE_LEN];
};

/* Fill every array and the capture buffer with the 0xDEAD pattern. */
void td_poison_vdi(struct td_vdi_arrays *a);
void td_poison_aes(struct td_aes_arrays *a);
void td_poison_call(struct td_call_frame *f);

/* Writers.  `rec` must be at least the area's record size. */
void td_vdi_record_write(uint8_t *rec, uint16_t case_id, uint16_t opcode,
                         int32_t ret, const struct td_vdi_arrays *after,
                         const int16_t *contrl_before);
void td_aes_record_write(uint8_t *rec, uint16_t case_id, uint16_t opcode,
                         int32_t ret, const struct td_aes_arrays *after);
void td_call_record_write(uint8_t *rec, uint16_t case_id, uint16_t opcode,
                          const struct td_call_frame *f);

/* ---- field descriptors (used by the comparator to name a difference) --- */

struct td_field {
	const char *name;
	uint16_t    off;    /* byte offset inside the record          */
	uint16_t    count;  /* element count                          */
	uint8_t     width;  /* element width in bytes: 2 or 4         */
	uint8_t     is_signed;
};

/* NULL-name-terminated tables. */
extern const struct td_field td_vdi_fields[];
extern const struct td_field td_aes_fields[];
extern const struct td_field td_call_fields[];

const struct td_field *td_area_fields(uint16_t area);

#endif /* TOSDUMP_RECORD_H */
