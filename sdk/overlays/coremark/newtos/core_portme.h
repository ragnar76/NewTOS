/*
Copyright 2018 Embedded Microprocessor Benchmark Consortium (EEMBC)

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

Original Author: Shay Gal-on
*/

/*
 * NewTOS/AArch64 port of CoreMark 1.0 -- porting layer configuration.
 *
 * CoreMark's run rules put the porting layer -- and ONLY the porting layer --
 * under the porter's control:
 *
 *   ALLOWED:      changing core_portme.h, core_portme.c, core_portme.mak,
 *                 the method of acquiring seed values, the method of acquiring
 *                 the data memory block, the toolchain and its options, and
 *                 the number of iterations.
 *   NOT ALLOWED:  changing any other source file.
 *
 * So this is a plain CoreMark port directory -- the same role as
 * sdk/apps/coremark/{atari,linux,barebones}/ -- and the five benchmark
 * translation units plus coremark.h are compiled straight out of the vendored
 * tree, untouched. sdk/apps/build-coremark.sh MD5-checks all six against
 * sdk/apps/coremark-sources.md5 before it compiles anything, so "we changed
 * nothing else" is a verified fact rather than a claim.
 *
 * It lives under sdk/overlays/coremark/ and not inside the vendored tree for
 * the same reason QED's memory.c does: sdk/apps/<app>/ is third-party source,
 * untracked by this repository (sdk/.gitignore), so anything of ours has to
 * live outside it and be reached at build time.
 *
 * Base: sdk/apps/coremark/atari/core_portme.h (Christian Zietz' Atari/TOS
 * port -- the closest relative: same TOS API, same 200 Hz clock()).
 * Deltas against it, each with its reason:
 *
 *   1. ee_ptr_int is a 64-bit, pointer-sized type.
 *      NewTOS applications are LP64 (docs/MEMORY.md, "App ABI: PIVOTED to
 *      LP64"): pointers are 64-bit while `int` stays 32-bit. coremark.h warns
 *      that ee_ptr_int "needs to be the data type used to hold pointers,
 *      otherwise coremark may fail", and core_util.c's check_data_types()
 *      enforces sizeof(ee_ptr_int) == sizeof(int *). The 68k port could use
 *      `unsigned int` because m68k is ILP32; we cannot.
 *      It is spelled __UINTPTR_TYPE__ and not uintptr_t -- see the note at
 *      the typedef: when this port was written our mintlib's <stdint.h> still
 *      carried the m68k definition (a 32-bit uintptr_t; fixed in TASK/PORT10).
 *
 *   2. SEED_METHOD is SEED_VOLATILE, not SEED_ARG.
 *      The Atari port takes its seeds from the command line. NewTOS has no
 *      way to hand a program a command tail interactively: the desktop
 *      launches a `.NCP` with an empty tail (programs/desktop/src/main.rs,
 *      launch_native) and there is no command-line shell. SEED_VOLATILE is
 *      CoreMark's own answer for exactly that case -- the README run rules
 *      spell it out: "If not using command line arguments to main: make
 *      XCFLAGS=-DPERFORMANCE_RUN=1 ... / -DVALIDATION_RUN=1 ...". Hence two
 *      executables (CORE.NCP / COREVAL.NCP) instead of one with arguments.
 *
 *   3. COMPILER_VERSION knows about clang.
 *      The template's fallback is "GCC" __VERSION__, which under our clang
 *      would report the literal string "GCCApple LLVM 21.0.0 ...". The run
 *      rules require the compiler version to be REPORTED correctly, so name
 *      the compiler that actually built the binary.
 *
 * Everything else (HAS_FLOAT/HAS_TIME_H/USE_CLOCK/HAS_STDIO/HAS_PRINTF,
 * MEM_METHOD, MEM_LOCATION, MULTITHREAD, the data types, align_mem) is the
 * Atari port unchanged -- which is what makes a NewTOS number and an Atari
 * TT number comparable in the first place.
 */

#ifndef CORE_PORTME_H
#define CORE_PORTME_H

#include <stddef.h> /* size_t -- for ee_size_t */

/************************/
/* Data types and settings */
/************************/
/* Configuration: HAS_FLOAT
        Define to 1 if the platform supports floating point.

        AArch64 has hardware double precision and the kernel sets
        CPACR_EL1.FPEN so EL0 may use FP/SIMD
        (kernel/src/arch/aarch64/boot.rs). mintlib's printf carries the full
        floating-point formatter (stdio/printf_fp.c), so "%f" prints real
        digits.
*/
#ifndef HAS_FLOAT
#define HAS_FLOAT 1
#endif
/* Configuration: HAS_TIME_H
        Define to 1 if platform has the time.h header file,
        and implementation of functions thereof.
*/
#ifndef HAS_TIME_H
#define HAS_TIME_H 1
#endif
/* Configuration: USE_CLOCK
        Define to 1 if platform has the time.h header file,
        and implementation of functions thereof.

        NewTOS runs mintlib's own, unmodified time/clock.c, which reads the
        classic Atari 200 Hz system variable _hz_200 ($4ba); CLOCKS_PER_SEC is
        200, exactly as on an Atari. Only DIFFERENCES of clock() are used by
        this port (see MYTIMEDIFF in core_portme.c), which matters because our
        crt0 does not run .init_array, so mintlib's clock() counts from BOOT
        rather than from program start. A difference is unaffected by that
        offset.
*/
#ifndef USE_CLOCK
#define USE_CLOCK 1
#endif
/* Configuration: HAS_STDIO
        Define to 1 if the platform has stdio.h.
*/
#ifndef HAS_STDIO
#define HAS_STDIO 1
#endif
/* Configuration: HAS_PRINTF
        Define to 1 if the platform has stdio.h and implements the printf
   function.
*/
#ifndef HAS_PRINTF
#define HAS_PRINTF 1
#endif

/* Configuration: CORE_TICKS
        Define type of return from the timing functions.
 */
#if HAS_TIME_H
#include <time.h>
typedef clock_t CORE_TICKS;
#else
#error "Please define type of CORE_TICKS and implement start_time, end_time get_time and time_in_secs functions!"
#endif

/* Definitions: COMPILER_VERSION, COMPILER_FLAGS, MEM_LOCATION
        Initialize these strings per platform
*/
#ifndef COMPILER_VERSION
#ifdef __clang__
#ifdef __apple_build_version__
#define COMPILER_VERSION "Apple clang " __clang_version__
#else
#define COMPILER_VERSION "clang " __clang_version__
#endif
#elif defined(__GNUC__)
#define COMPILER_VERSION "GCC" __VERSION__
#else
#error "Define COMPILER_VERSION: CoreMark's run rules require reporting it."
#endif
#endif
#ifndef COMPILER_FLAGS
#define COMPILER_FLAGS FLAGS_STR /* set by build-coremark.sh via -DFLAGS_STR */
#endif
#ifndef MEM_LOCATION
#define MEM_LOCATION "Code and data in RAM"
#endif

/* Data Types:
        To avoid compiler issues, define the data types that need ot be used
   for 8b, 16b and 32b in <core_portme.h>.

        *Imprtant*:
        ee_ptr_int needs to be the data type used to hold pointers, otherwise
   coremark may fail!!!
*/
typedef signed short   ee_s16;
typedef unsigned short ee_u16;
typedef signed int     ee_s32;
typedef double         ee_f32;
typedef unsigned char  ee_u8;
typedef unsigned int   ee_u32;
/* ee_ptr_int: the pointer-sized unsigned integer, taken from the COMPILER
   (__UINTPTR_TYPE__ = `long unsigned int` for aarch64-unknown-none-elf) and
   deliberately NOT from <stdint.h>.

   When this port was written, our mintlib's include/stdint.h still carried
   its m68k definition:

       #ifdef __MSHORT__
       typedef unsigned long int uintptr_t;
       #else
       typedef unsigned int      uintptr_t;   <-- 32 bit, taken on AArch64
       #endif

   i.e. on our LP64 target uintptr_t was 32 bits wide and silently truncated a
   pointer; using it here would have made check_data_types() report an error
   and corrupted align_mem()'s result. TASK/PORT10 fixed the libc
   (sdk/libc/overlay/include/stdint.h: `unsigned long' on LP64), so the two
   spellings now agree; this one stays because it never depended on the
   libc. */
typedef __UINTPTR_TYPE__ ee_ptr_int;
typedef size_t           ee_size_t;
/* align_mem:
        This macro is used to align an offset to point to a 32b value. It is
   used in the Matrix algorithm to initialize the input memory blocks.
*/
#define align_mem(x) (void *)(4 + (((ee_ptr_int)(x)-1) & ~3))

/* Configuration: SEED_METHOD
        Defines method to get seed values that cannot be computed at compile
   time.

        Valid values:
        SEED_ARG - from command line.
        SEED_FUNC - from a system function.
        SEED_VOLATILE - from volatile variables.

        SEED_VOLATILE here; see delta 2 in the file header. PERFORMANCE_RUN /
        VALIDATION_RUN select which seed set core_portme.c compiles in, and
        build-coremark.sh builds one executable for each.
*/
#ifndef SEED_METHOD
#define SEED_METHOD SEED_VOLATILE
#endif

/* Configuration: MEM_METHOD
        Defines method to get a block of memry.

        Valid values:
        MEM_MALLOC - for platforms that implement malloc and have malloc.h.
        MEM_STATIC - to use a static memory array.
        MEM_STACK - to allocate the data block on the stack (NYI).

        MEM_MALLOC: mintlib's malloc runs on the free TPA region the loader
        gives the process (sdk/libc/newtos-arch/heap_init.c), i.e. ordinary
        RAM -- so the reported result needs no memory qualifier.
*/
#ifndef MEM_METHOD
#define MEM_METHOD MEM_MALLOC
#endif

/* Configuration: MULTITHREAD
        Define for parallel execution

        Valid values:
        1 - only one context (default).
        N>1 - will execute N copies in parallel.

        NewTOS is single-tasking (docs/MEMORY.md, "Single-tasking: app owns
        full screen"): there is no fork, no pthreads, and no second process.
        MULTITHREAD stays 1; core_portme.c raises an #error if it does not,
        rather than pretending to start contexts.
*/
#ifndef MULTITHREAD
#define MULTITHREAD 1
#endif

/* Configuration: USE_PTHREAD / USE_FORK / USE_SOCKET
        Sample implementations for launching parallel contexts. None applies
        to a single-tasking kernel; all off.
*/
#ifndef USE_PTHREAD
#define USE_PTHREAD 0
#endif
#ifndef USE_FORK
#define USE_FORK 0
#endif
#ifndef USE_SOCKET
#define USE_SOCKET 0
#endif

/* Configuration: MAIN_HAS_NOARGC
        Needed if platform does not support getting arguments to main.

        Valid values:
        0 - argc/argv to main is supported
        1 - argc/argv to main is not supported

        sdk/libc/crt0.S calls main(1, {"", NULL}), so main(argc, argv) is a
        legal signature and portable_init() may read them. The values carry no
        seeds (delta 2), which is why SEED_METHOD is not SEED_ARG.
*/
#ifndef MAIN_HAS_NOARGC
#define MAIN_HAS_NOARGC 0
#endif

/* Configuration: MAIN_HAS_NORETURN
        Needed if platform does not support returning a value from main.

        crt0 passes main's return value straight to GEMDOS Pterm (0x4C), so
        returning a value is supported.
*/
#ifndef MAIN_HAS_NORETURN
#define MAIN_HAS_NORETURN 0
#endif

/* Variable: default_num_contexts
        Number of contexts to spawn in multicore context.
        Override this global value to change number of contexts used.
*/
extern ee_u32 default_num_contexts;

typedef struct CORE_PORTABLE_S
{
    ee_u8 portable_id;
} core_portable;

/* target specific init/fini */
void portable_init(core_portable *p, int *argc, char *argv[]);
void portable_fini(core_portable *p);

#if (SEED_METHOD == SEED_VOLATILE)
#if (VALIDATION_RUN || PERFORMANCE_RUN || PROFILE_RUN)
#define RUN_TYPE_FLAG 1
#else
#if (TOTAL_DATA_SIZE == 1200)
#define PROFILE_RUN 1
#else
#define PERFORMANCE_RUN 1
#endif
#endif
#endif /* SEED_METHOD==SEED_VOLATILE */

#endif /* CORE_PORTME_H */
