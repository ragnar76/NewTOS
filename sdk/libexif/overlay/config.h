/* config.h for libexif 0.6.26 on NewTOS/AArch64 (TASK/LDG-M2).
 *
 * WHY THIS FILE EXISTS: libexif's config.h is not part of the release; its
 * ./configure writes it from config.h.in (AC_CONFIG_HEADERS, configure.ac:13).
 * We do not run configure for a freestanding cross target, so this is
 * config.h.in with every answer filled in for aarch64-unknown-none-elf +
 * mintlib, each with its reason. The library sources stay unmodified.
 * Only ENABLE_NLS, GETTEXT_PACKAGE and HAVE_LOCALTIME_R/_S are read by the
 * library sources (grep over libexif/); the rest is kept so the file is a
 * complete config.h.
 */

/* No gettext: libexif's i18n.h then maps _(), dgettext() and
 * bindtextdomain() to the untranslated strings (English tag names/values --
 * what zView's JPEG codec shows in its info window). */
/* #undef ENABLE_NLS */

/* configure.ac:252 GP_GETTEXT_HACK([${PACKAGE}-${LIBEXIF_CURRENT_MIN}]) with
 * LIBEXIF_CURRENT=15, LIBEXIF_AGE=3 (configure.ac:66-72). Without NLS it is
 * only the (discarded) first argument of bindtextdomain(), but it must be
 * defined for exif-tag.c & co. to compile. */
#define GETTEXT_PACKAGE "libexif-12"

/* mintlib declares and implements localtime_r() (time.h:202, in libc.a);
 * exif-entry.c:1723 prefers it to localtime(). localtime_s is Windows. */
#define HAVE_LOCALTIME_R 1
/* #undef HAVE_LOCALTIME_S */

/* Standard headers mintlib provides. */
#define HAVE_INTTYPES_H 1
#define HAVE_STDINT_H 1
#define HAVE_STDIO_H 1
#define HAVE_STDLIB_H 1
#define HAVE_STRINGS_H 1
#define HAVE_STRING_H 1
#define HAVE_SYS_STAT_H 1
#define HAVE_SYS_TYPES_H 1
#define HAVE_UNISTD_H 1
#define STDC_HEADERS 1

/* Not available / not applicable on NewTOS. */
/* #undef HAVE_CFLOCALECOPYCURRENT */
/* #undef HAVE_CFPREFERENCESCOPYAPPVALUE */
/* #undef HAVE_DCGETTEXT */
/* #undef HAVE_DLFCN_H */
/* #undef HAVE_GETTEXT */
/* #undef HAVE_ICONV */
/* #undef ICONV_CONST */

/* AC_INIT, configure.ac:7-11. */
#define PACKAGE "libexif"
#define PACKAGE_BUGREPORT "libexif-devel@lists.sourceforge.net"
#define PACKAGE_NAME "EXIF library"
#define PACKAGE_STRING "EXIF library 0.6.26"
#define PACKAGE_TARNAME "libexif"
#define PACKAGE_URL "https://libexif.github.io/"
#define PACKAGE_VERSION "0.6.26"
#define VERSION "0.6.26"

/* #undef LT_OBJDIR */
/* #undef _FILE_OFFSET_BITS */
/* #undef _LARGE_FILES */
/* #undef _TIME_BITS */
/* #undef __MINGW_USE_VC2005_COMPAT */
/* "inline" is a keyword for clang in the C dialect used; no remap. */
