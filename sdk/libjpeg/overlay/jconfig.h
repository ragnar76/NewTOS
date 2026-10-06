/* jconfig.h for NewTOS/AArch64 (TASK/LDG-M2).
 *
 * WHY THIS FILE EXISTS: IJG's jconfig.h is not part of the release; it is
 * written by ./configure from jconfig.cfg (or copied by hand from one of the
 * jconfig.* templates, see install.txt). We do not run configure for a
 * freestanding cross target, so this is jconfig.cfg with every answer filled
 * in for aarch64-unknown-none-elf + mintlib, each one stated with its reason.
 * The library sources stay unmodified.
 */

#define HAVE_PROTOTYPES         /* ISO C compiler */
#define HAVE_UNSIGNED_CHAR
#define HAVE_UNSIGNED_SHORT
/* #undef void */
/* #undef const */

/* Plain char is UNSIGNED on AArch64 (AAPCS64; clang defines
 * __CHAR_UNSIGNED__ for aarch64-unknown-none-elf). configure's test program
 * would find the same. It only matters when HAVE_UNSIGNED_CHAR is NOT defined
 * (jmorecfg.h:72-79, 165-174, 186-194); with HAVE_UNSIGNED_CHAR, as here,
 * JSAMPLE/JOCTET/UINT8 are unsigned char either way. Set for accuracy. */
#define CHAR_IS_UNSIGNED

#define HAVE_STDDEF_H           /* mintlib has <stddef.h> (via clang's) */
#define HAVE_STDLIB_H           /* mintlib <stdlib.h>, declares getenv() */
/* #undef HAVE_LOCALE_H */      /* used by cjpeg/djpeg only, not the library */
/* #undef NEED_BSD_STRINGS */   /* mintlib has <string.h> with memcpy/memset */
/* #undef NEED_SYS_TYPES_H */   /* size_t comes from <stddef.h> */
/* #undef NEED_FAR_POINTERS */  /* flat 64-bit address space */
/* #undef NEED_SHORT_EXTERNAL_NAMES */
/* #undef INCOMPLETE_TYPES_BROKEN */

/* "boolean" stays jmorecfg.h's default (int); the _WIN32 block of
 * jconfig.cfg does not apply. */

#ifdef JPEG_INTERNALS

/* >> on a negative signed value is arithmetic on AArch64 (ASR); clang
 * guarantees it. */
/* #undef RIGHT_SHIFT_IS_UNSIGNED */
/* #undef INLINE */             /* jmorecfg.h's default */
/* Memory manager: jmemnobs.c (configure's default without --enable-maxmem,
 * configure.ac "MEMORYMGR='jmemnobs'"): plain malloc/free, no temp files.
 * DEFAULT_MAX_MEM / NO_MKTEMP only matter for jmemansi/jmemname. */
/* #undef DEFAULT_MAX_MEM */
/* #undef NO_MKTEMP */

#endif /* JPEG_INTERNALS */

#ifdef JPEG_CJPEG_DJPEG
/* The command-line tools are not built for NewTOS; jconfig.cfg's defaults are
 * kept so that the file is a complete jconfig.h. */
#define BMP_SUPPORTED
#define GIF_SUPPORTED
#define PPM_SUPPORTED
/* #undef RLE_SUPPORTED */
#define TARGA_SUPPORTED
/* #undef TWO_FILE_COMMANDLINE */
/* #undef NEED_SIGNAL_CATCHER */
/* #undef DONT_USE_B_MODE */
/* #undef PROGRESS_REPORT */
#endif /* JPEG_CJPEG_DJPEG */
