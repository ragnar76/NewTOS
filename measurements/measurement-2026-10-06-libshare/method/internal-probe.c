/* Link probe (TASK/LDG-M3-B2): a "module" that names mintlib's allocator
   internals (__malloc, _mallocChunkSize is libshare's too, __malloc is
   not). The link must SUCCEED at the linker level and the libshare map
   check must FAIL it -- that is what sdk/libshare/libshare-link.sh is for. */
#include <stdlib.h>
extern void *__malloc (size_t);
int main (void) { void *p = __malloc (10); free (p); return 0; }
