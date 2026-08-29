/*
 * record.c -- big-endian record writer / reader for the TOS reference dumper.
 *
 * Spec: docs/TOS-REFERENCE-DUMPER.md sections 2.1, 3 and 4.
 *
 * Compiled by all three toolchains (host cc, m68k-atari-mint-gcc,
 * clang/aarch64).  No struct is ever written to the file: every field goes to
 * an explicit offset through td_store_be*, so padding and alignment rules
 * cannot differ between targets.
 */
#include <string.h>

#include "record.h"

/* ------------------------------------------------------------------ */
/* Byte order (spec section 2.1)                                       */
/* ------------------------------------------------------------------ */
/*
 * Section 2.1 frames this as "the AArch64 build byte-swaps on write, the m68k
 * build does not" -- a compile-time selection around a store of the machine's
 * own native word.  The same section also requires a host-side test proving
 * that both configurations emit byte-identical records, which a native-word
 * store cannot satisfy: on any one host only one of the two configurations
 * would be correct.
 *
 * The two requirements together settle the implementation.  Both branches
 * below are written with explicit byte indices, so each is correct on any
 * machine, and the little-endian branch spells out exactly what it models --
 * swap the word, then store it low byte first, which is what a native store on
 * a little-endian machine does.  TD_BIG_ENDIAN_TARGET is still honoured, so
 * the two configurations are really built and really compared; it selects
 * between two spellings that must agree byte for byte.
 */
#ifndef TD_BIG_ENDIAN_TARGET
#  if defined(__BYTE_ORDER__) && defined(__ORDER_BIG_ENDIAN__) && \
      (__BYTE_ORDER__ == __ORDER_BIG_ENDIAN__)
#    define TD_BIG_ENDIAN_TARGET 1
#  else
#    define TD_BIG_ENDIAN_TARGET 0
#  endif
#endif

void td_store_be16(uint8_t *p, uint16_t v)
{
#if TD_BIG_ENDIAN_TARGET
	/* m68k: the record order is already the machine order, no swap. */
	p[0] = (uint8_t)(v >> 8);
	p[1] = (uint8_t)(v & 0xFFu);
#else
	/* AArch64: swap on write, then store low byte first. */
	uint16_t s = (uint16_t)(((v & 0x00FFu) << 8) | ((v & 0xFF00u) >> 8));
	p[0] = (uint8_t)(s & 0xFFu);
	p[1] = (uint8_t)((s >> 8) & 0xFFu);
#endif
}

void td_store_be32(uint8_t *p, uint32_t v)
{
#if TD_BIG_ENDIAN_TARGET
	p[0] = (uint8_t)((v >> 24) & 0xFFuL);
	p[1] = (uint8_t)((v >> 16) & 0xFFuL);
	p[2] = (uint8_t)((v >>  8) & 0xFFuL);
	p[3] = (uint8_t)( v        & 0xFFuL);
#else
	uint32_t s = ((v & 0x000000FFuL) << 24) |
	             ((v & 0x0000FF00uL) <<  8) |
	             ((v & 0x00FF0000uL) >>  8) |
	             ((v & 0xFF000000uL) >> 24);
	p[0] = (uint8_t)( s        & 0xFFuL);
	p[1] = (uint8_t)((s >>  8) & 0xFFuL);
	p[2] = (uint8_t)((s >> 16) & 0xFFuL);
	p[3] = (uint8_t)((s >> 24) & 0xFFuL);
#endif
}

uint16_t td_load_be16(const uint8_t *p)
{
	return (uint16_t)(((uint16_t)p[0] << 8) | (uint16_t)p[1]);
}

uint32_t td_load_be32(const uint8_t *p)
{
	return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) |
	       ((uint32_t)p[2] <<  8) |  (uint32_t)p[3];
}

/* ------------------------------------------------------------------ */
/* CRC32 -- IEEE 802.3, reflected, the value crc32(1) prints            */
/* ------------------------------------------------------------------ */

uint32_t td_crc32(const uint8_t *data, size_t len)
{
	uint32_t crc = 0xFFFFFFFFuL;
	size_t i;
	int b;

	for (i = 0; i < len; i++) {
		crc ^= (uint32_t)data[i];
		for (b = 0; b < 8; b++)
			crc = (crc >> 1) ^ (0xEDB88320uL & (uint32_t)(-(int32_t)(crc & 1uL)));
	}
	return crc ^ 0xFFFFFFFFuL;
}

/* ------------------------------------------------------------------ */
/* File header                                                         */
/* ------------------------------------------------------------------ */

uint16_t td_area_record_size(uint16_t area)
{
	switch (area) {
	case TD_AREA_BIOS:
	case TD_AREA_XBIOS:
	case TD_AREA_GEMDOS: return TD_CALL_RECORD_SIZE;
	case TD_AREA_VDI:    return TD_VDI_RECORD_SIZE;
	case TD_AREA_AES:    return TD_AES_RECORD_SIZE;
	default:             return 0;
	}
}

const char *td_area_name(uint16_t area)
{
	switch (area) {
	case TD_AREA_BIOS:   return "BIOS";
	case TD_AREA_XBIOS:  return "XBIOS";
	case TD_AREA_GEMDOS: return "GEMDOS";
	case TD_AREA_VDI:    return "VDI";
	case TD_AREA_AES:    return "AES";
	default:             return "?";
	}
}

void td_header_write(uint8_t *hdr, const struct td_header *h)
{
	int i;

	memset(hdr, 0, TD_HEADER_SIZE);
	memcpy(hdr + TD_HDR_OFF_MAGIC, TD_MAGIC, TD_MAGIC_LEN);
	td_store_be16(hdr + TD_HDR_OFF_VERSION,  h->version);
	td_store_be16(hdr + TD_HDR_OFF_AREA,     h->area);
	td_store_be16(hdr + TD_HDR_OFF_RECSIZE,  h->record_size);
	td_store_be16(hdr + TD_HDR_OFF_RECCOUNT, h->record_count);

	/* build id: ASCII, zero padded -- never left uninitialised */
	for (i = 0; i < TD_HDR_BUILDID_LEN; i++) {
		char c = h->build_id[i];
		hdr[TD_HDR_OFF_BUILDID + i] = (uint8_t)c;
		if (c == '\0')
			break;
	}

	td_store_be32(hdr + TD_HDR_OFF_CASECRC, h->case_crc);
	/* reserved stays zero from the memset above */
}

int td_header_parse(const uint8_t *hdr, size_t len, struct td_header *out)
{
	uint16_t mandated;
	int i;

	if (len < TD_HEADER_SIZE)
		return TD_ERR_SIZE;
	if (memcmp(hdr + TD_HDR_OFF_MAGIC, TD_MAGIC, TD_MAGIC_LEN) != 0)
		return TD_ERR_MAGIC;

	out->version = td_load_be16(hdr + TD_HDR_OFF_VERSION);
	if (out->version != TD_FORMAT_VERSION)
		return TD_ERR_VERSION;

	for (i = 0; i < TD_HDR_RESERVED_LEN; i++)
		if (hdr[TD_HDR_OFF_RESERVED + i] != 0)
			return TD_ERR_RESERVED;

	out->area = td_load_be16(hdr + TD_HDR_OFF_AREA);
	mandated = td_area_record_size(out->area);
	if (mandated == 0)
		return TD_ERR_AREA;

	out->record_size = td_load_be16(hdr + TD_HDR_OFF_RECSIZE);
	if (out->record_size != mandated)
		return TD_ERR_RECSIZE;

	out->record_count = td_load_be16(hdr + TD_HDR_OFF_RECCOUNT);
	memcpy(out->build_id, hdr + TD_HDR_OFF_BUILDID, TD_HDR_BUILDID_LEN);
	out->build_id[TD_HDR_BUILDID_LEN] = '\0';
	out->case_crc = td_load_be32(hdr + TD_HDR_OFF_CASECRC);
	return 0;
}

/* ------------------------------------------------------------------ */
/* Poison (spec section 4)                                             */
/* ------------------------------------------------------------------ */

static void poison16(int16_t *p, int n)
{
	int i;
	for (i = 0; i < n; i++)
		p[i] = TD_POISON16;
}

static void poison32(uint32_t *p, int n)
{
	int i;
	for (i = 0; i < n; i++)
		p[i] = 0xDEADDEADuL;
}

void td_poison_vdi(struct td_vdi_arrays *a)
{
	poison16(a->contrl, TD_VDI_CONTRL_N);
	poison16(a->intin,  TD_VDI_INTIN_N);
	poison16(a->ptsin,  TD_VDI_PTSIN_N);
	poison16(a->intout, TD_VDI_INTOUT_N);
	poison16(a->ptsout, TD_VDI_PTSOUT_N);
}

void td_poison_aes(struct td_aes_arrays *a)
{
	poison16(a->contrl,  TD_AES_CONTRL_N);
	poison16(a->global,  TD_AES_GLOBAL_N);
	poison16(a->int_in,  TD_AES_INTIN_N);
	poison16(a->int_out, TD_AES_INTOUT_N);
	poison32(a->addr_in,  TD_AES_ADDRIN_N);
	poison32(a->addr_out, TD_AES_ADDROUT_N);
}

void td_poison_call(struct td_call_frame *f)
{
	int i;

	poison32(f->args, TD_CALL_ARGS_N);
	f->ret = (int32_t)0xDEADDEADuL;
	for (i = 0; i < TD_CALL_CAPTURE_LEN; i += 2) {
		f->capture[i]     = TD_POISON_HI;
		f->capture[i + 1] = TD_POISON_LO;
	}
}

/* ------------------------------------------------------------------ */
/* Record writers                                                      */
/* ------------------------------------------------------------------ */

static void store16_array(uint8_t *dst, const int16_t *src, int n)
{
	int i;
	for (i = 0; i < n; i++)
		td_store_be16(dst + i * 2, (uint16_t)src[i]);
}

static void store32_array(uint8_t *dst, const uint32_t *src, int n)
{
	int i;
	for (i = 0; i < n; i++)
		td_store_be32(dst + i * 4, src[i]);
}

void td_vdi_record_write(uint8_t *rec, uint16_t case_id, uint16_t opcode,
                         int32_t ret, const struct td_vdi_arrays *after,
                         const int16_t *contrl_before)
{
	memset(rec, 0, TD_VDI_RECORD_SIZE);
	td_store_be16(rec + TD_VDI_OFF_CASEID, case_id);
	td_store_be16(rec + TD_VDI_OFF_OPCODE, opcode);
	td_store_be32(rec + TD_VDI_OFF_RET, (uint32_t)ret);
	store16_array(rec + TD_VDI_OFF_CONTRL, after->contrl, TD_VDI_CONTRL_N);
	store16_array(rec + TD_VDI_OFF_INTIN,  after->intin,  TD_VDI_INTIN_N);
	store16_array(rec + TD_VDI_OFF_PTSIN,  after->ptsin,  TD_VDI_PTSIN_N);
	store16_array(rec + TD_VDI_OFF_INTOUT, after->intout, TD_VDI_INTOUT_N);
	store16_array(rec + TD_VDI_OFF_PTSOUT, after->ptsout, TD_VDI_PTSOUT_N);
	store16_array(rec + TD_VDI_OFF_CONTRL_BEFORE, contrl_before,
	              TD_VDI_CONTRL_N);
}

void td_aes_record_write(uint8_t *rec, uint16_t case_id, uint16_t opcode,
                         int32_t ret, const struct td_aes_arrays *after)
{
	memset(rec, 0, TD_AES_RECORD_SIZE);
	td_store_be16(rec + TD_AES_OFF_CASEID, case_id);
	td_store_be16(rec + TD_AES_OFF_OPCODE, opcode);
	store16_array(rec + TD_AES_OFF_CONTRL,  after->contrl,  TD_AES_CONTRL_N);
	store16_array(rec + TD_AES_OFF_GLOBAL,  after->global,  TD_AES_GLOBAL_N);
	store16_array(rec + TD_AES_OFF_INTIN,   after->int_in,  TD_AES_INTIN_N);
	store16_array(rec + TD_AES_OFF_INTOUT,  after->int_out, TD_AES_INTOUT_N);
	store32_array(rec + TD_AES_OFF_ADDRIN,  after->addr_in,  TD_AES_ADDRIN_N);
	store32_array(rec + TD_AES_OFF_ADDROUT, after->addr_out, TD_AES_ADDROUT_N);
	td_store_be32(rec + TD_AES_OFF_RET, (uint32_t)ret);
	/* reserved stays zero from the memset above */
}

void td_call_record_write(uint8_t *rec, uint16_t case_id, uint16_t opcode,
                          const struct td_call_frame *f)
{
	memset(rec, 0, TD_CALL_RECORD_SIZE);
	td_store_be16(rec + TD_CALL_OFF_CASEID, case_id);
	td_store_be16(rec + TD_CALL_OFF_OPCODE, opcode);
	store32_array(rec + TD_CALL_OFF_ARGS, f->args, TD_CALL_ARGS_N);
	td_store_be32(rec + TD_CALL_OFF_RET, (uint32_t)f->ret);
	memcpy(rec + TD_CALL_OFF_CAPTURE, f->capture, TD_CALL_CAPTURE_LEN);
	/* reserved stays zero from the memset above */
}

/* ------------------------------------------------------------------ */
/* Field descriptors -- so a difference is reported by name             */
/* ------------------------------------------------------------------ */

const struct td_field td_vdi_fields[] = {
	{ "d0",            TD_VDI_OFF_RET,           1,               4, 1 },
	{ "contrl",        TD_VDI_OFF_CONTRL,        TD_VDI_CONTRL_N, 2, 1 },
	{ "intin",         TD_VDI_OFF_INTIN,         TD_VDI_INTIN_N,  2, 1 },
	{ "ptsin",         TD_VDI_OFF_PTSIN,         TD_VDI_PTSIN_N,  2, 1 },
	{ "intout",        TD_VDI_OFF_INTOUT,        TD_VDI_INTOUT_N, 2, 1 },
	{ "ptsout",        TD_VDI_OFF_PTSOUT,        TD_VDI_PTSOUT_N, 2, 1 },
	{ "contrl_before", TD_VDI_OFF_CONTRL_BEFORE, TD_VDI_CONTRL_N, 2, 1 },
	{ NULL, 0, 0, 0, 0 }
};

const struct td_field td_aes_fields[] = {
	{ "d0",       TD_AES_OFF_RET,     1,                4, 1 },
	{ "contrl",   TD_AES_OFF_CONTRL,  TD_AES_CONTRL_N,  2, 1 },
	{ "global",   TD_AES_OFF_GLOBAL,  TD_AES_GLOBAL_N,  2, 1 },
	{ "int_in",   TD_AES_OFF_INTIN,   TD_AES_INTIN_N,   2, 1 },
	{ "int_out",  TD_AES_OFF_INTOUT,  TD_AES_INTOUT_N,  2, 1 },
	{ "addr_in",  TD_AES_OFF_ADDRIN,  TD_AES_ADDRIN_N,  4, 0 },
	{ "addr_out", TD_AES_OFF_ADDROUT, TD_AES_ADDROUT_N, 4, 0 },
	{ NULL, 0, 0, 0, 0 }
};

const struct td_field td_call_fields[] = {
	{ "args",    TD_CALL_OFF_ARGS,    TD_CALL_ARGS_N,      4, 0 },
	{ "ret",     TD_CALL_OFF_RET,     1,                   4, 1 },
	{ "capture", TD_CALL_OFF_CAPTURE, TD_CALL_CAPTURE_LEN, 1, 0 },
	{ NULL, 0, 0, 0, 0 }
};

const struct td_field *td_area_fields(uint16_t area)
{
	switch (area) {
	case TD_AREA_BIOS:
	case TD_AREA_XBIOS:
	case TD_AREA_GEMDOS: return td_call_fields;
	case TD_AREA_VDI:    return td_vdi_fields;
	case TD_AREA_AES:    return td_aes_fields;
	default:             return NULL;
	}
}
