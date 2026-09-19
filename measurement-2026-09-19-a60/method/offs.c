#include <stddef.h>
typedef short _WORD; typedef unsigned short _UWORD; typedef long _LONG_PTR;
typedef struct { int x; } OBJECT;
typedef union { void *spec; _LONG_PTR l; short pi[2]; } aes_private;
typedef struct _aes_global {
	_WORD ap_version; _WORD ap_count; _WORD ap_id; aes_private ap_private;
	OBJECT **ap_ptree; void *ap_rscmem; _UWORD ap_rsclen; _WORD ap_planes;
	void *ap_3resv; _WORD ap_bvdisk; _WORD ap_bvhard;
} AES_GLOBAL;
#define P(f) __attribute__((used)) static const int off_##f = offsetof(AES_GLOBAL, f);
P(ap_version) P(ap_count) P(ap_id) P(ap_private) P(ap_ptree) P(ap_rscmem) P(ap_rsclen) P(ap_planes) P(ap_3resv) P(ap_bvdisk) P(ap_bvhard)
__attribute__((used)) static const int size_all = sizeof(AES_GLOBAL);
