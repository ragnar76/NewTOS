/*
 * sysacc.c -- SYSINFO02: the "System Info" desk accessory, version 2.
 *
 * A GEM dialog that shows the state the kernel actually runs with -- the
 * machine (CPU clock, temperature, kernel version, ST-RAM / TT-RAM), the
 * display, the SD bus and the keyboard layout -- and the eight start-up
 * settings that live in the boot arguments (C:\CMDLINE.TXT on the Pi):
 * memory protection, address-space IDs, SD card mode, CPU clock, block
 * cache, cache share, pointer motion while a program computes, serial
 * console. For each setting the dialog shows the value the kernel runs with
 * NOW (asked through NewTOS's own Ssystem subfunctions, GEMDOS 0x154, modes
 * 2000-2019, docs/api/gemdos/154-ssystem.md) and the value selected for the
 * NEXT start (read from C:\CMDLINE.TXT the way the kernel reads its boot
 * arguments). Clicking a next-start value opens a small drop-down list of
 * the choices; Save writes C:\CMDLINE.TXT. The file's text is never shown:
 * every value is a plain-language label (user, 2026-10-07/08).
 *
 * A value the kernel does not report is shown as "not available", never as
 * an invented one. (The pointer-motion mode, `input=aes`, has no Ssystem
 * mode yet: its NOW column says so.)
 *
 * History: ACC06 built it as one form_alert (Ssystem 2000/2001); SYSINFO01
 * (2026-10-07) turned it into a read-only dialog; SYSINFO02 (2026-10-08)
 * made the start-up settings selectable and savable.
 *
 * FILE EXTENSION: `.NAC` (ADR-018, ACC05): a native AArch64 desk accessory.
 * The AES loads it once from C:\ACC\*.NAC and C:\*.NAC at session start
 * (docs/api/aes/accessories.md section 7); it is never double-clicked.
 *
 *     sdk/build.sh sdk/examples/sysacc.c /somewhere/SYSACC.NAC
 *
 * tools/make-sd-image.sh installs it into both search locations.
 *
 * ---------------------------------------------------------------------------
 * What it does
 * ---------------------------------------------------------------------------
 *   appl_init                       -> our application id
 *   menu_register(id, " System Info")
 *   forever:
 *       evnt_mesag
 *       AC_OPEN  (40) with our menu id -> read the values, run the dialog
 *       AC_CLOSE (41)                  -> nothing to tear down; keep running
 *
 * The dialog is the classic modal sequence (EmuTOS gemfmlib.c / the AES
 * reference): wind_update(BEG_UPDATE), form_center, form_dial(FMD_START),
 * objc_draw, form_do ..., form_dial(FMD_FINISH), wind_update(END_UPDATE).
 * form_do is called again after every exit object that does not close the
 * dialog (a drop-down button, Save).
 *
 * Buttons: Save writes the selection to C:\CMDLINE.TXT and keeps the dialog
 * open; OK closes it, after the same save when something was changed and not
 * yet saved; Cancel closes it and forgets unsaved selections. Before any
 * write one form_alert lists the changes in plain words.
 *
 * The drop-down: the AES's menu_popup (36) is not implemented in NewTOS yet,
 * so the accessory does what EmuTOS's mn_popup does (aes/gemmnext.c:302-345,
 * handle_popup :240-293) with public AES calls only: take the mouse
 * (wind_update BEG_MCTRL), wait for the button to go up, draw a small object
 * tree of its own over the clicked button (the current choice over the
 * button, CHECKED; the item under the pointer SELECTED), track the pointer
 * with evnt_multi(MU_M1 | MU_BUTTON | MU_KEYBD) -- M1 leaves the item under
 * the pointer or enters the box -- and end on a click (an item: choose it;
 * elsewhere: cancel), Esc (cancel) or Return (the highlighted item); the
 * cursor keys move the highlight. The box always lies inside the dialog, so
 * the dialog's own objc_draw, clipped to the box, restores what it covered
 * (EmuTOS saves and blits the background instead; the result on screen is
 * the same). Then wait for the button to go up and give the mouse back
 * (END_MCTRL).
 *
 * Saving (sysacc-cmdline.h has the token rules, tested on the host):
 *   1. the new text = the old C:\CMDLINE.TXT with every token the accessory
 *      does not manage kept in order, the managed ones replaced or removed,
 *      kernel defaults omitted, one line;
 *   2. C:\CMDLINE.BAK = a copy of the old file (when there was one);
 *   3. C:\CMDLINE.NEW = the new text, written and closed;
 *   4. Fdelete C:\CMDLINE.TXT, Frename C:\CMDLINE.NEW -> C:\CMDLINE.TXT
 *      (GEMDOS Frename does not replace an existing target).
 * A failure stops the sequence with an alert naming the step; the old file
 * is then still either in place or in C:\CMDLINE.BAK. A file the accessory
 * cannot read completely (read error, 4 KB or more) is never rewritten.
 * config.txt is never touched.
 *
 * Look: the AES 3D style (memory note aes-dialog-3d-look; the same flags
 * form_alert uses, tos-api/src/aes/form.rs): the root is an OUTLINED G_BOX
 * with FL3DBAK, buttons FL3DACT, the drop-down box FL3DBAK + SHADOWED.
 * 8x16 system font; the dialog is 612 x 452 pixels -- it fits a 640 x 480
 * screen below its menu bar.
 *
 * Calls used, all against docs/api/ and the EmuTOS AES:
 *   appl_init       AES  10      objc_change     AES  47
 *   evnt_button     AES  21      form_do         AES  50
 *   evnt_mesag      AES  23      form_dial       AES  51
 *   evnt_multi      AES  25      form_alert      AES  52
 *   menu_register   AES  35      form_center     AES  54
 *   objc_draw       AES  42      graf_mkstate    AES  79
 *   objc_find       AES  43      wind_update     AES 107
 *   objc_offset     AES  44
 *   Fcreate 0x3C, Fopen 0x3D, Fclose 0x3E, Fread 0x3F, Fwrite 0x40,
 *   Fdelete 0x41, Frename 0x56, Ssystem 0x154 (modes 2000-2019)
 */
#include <newtos.h>

#include "sysacc-cmdline.h"

/* ------------------------------------------------------------------------- */
/* GEMDOS: newtos.h ships gemdos0/1/2 only.                                   */
/* ------------------------------------------------------------------------- */

static inline NLONG gemdos3(NLONG fn, NLONG a0, NLONG a1, NLONG a2)
{
    register NLONG x8 __asm__("x8") = fn;
    register NLONG r0 __asm__("x0") = a0;
    register NLONG r1 __asm__("x1") = a1;
    register NLONG r2 __asm__("x2") = a2;
    __asm__ volatile("svc #1"
                     : "+r"(r0)
                     : "r"(x8), "r"(r1), "r"(r2)
                     : "memory");
    return r0;
}

#define E_OK    0L
#define EFILNF  (-33L)

/* File calls (docs/api/gemdos/file/). */
#define Fcreate(n, a)    gemdos2(0x3C, (NLONG)(n), (NLONG)(a))
#define Fopen(n, m)      gemdos2(0x3D, (NLONG)(n), (NLONG)(m))
#define Fclose(h)        gemdos1(0x3E, (NLONG)(h))
#define Fread(h, c, b)   gemdos3(0x3F, (NLONG)(h), (NLONG)(c), (NLONG)(b))
#define Fwrite(h, c, b)  gemdos3(0x40, (NLONG)(h), (NLONG)(c), (NLONG)(b))
#define Fdelete(n)       gemdos1(0x41, (NLONG)(n))
#define Frename(o, n)    gemdos3(0x56, 0, (NLONG)(o), (NLONG)(n))

#define SSYSTEM 0x154

/* NewTOS-own Ssystem modes (docs/api/gemdos/154-ssystem.md). */
#define S_NEWTOS_CPUCLOCK   2000 /* ARM clock, requested rate, Hz      */
#define S_NEWTOS_CPUTEMP    2001 /* SoC temperature, 1/1000 C          */
#define S_NEWTOS_CPUMEAS    2003 /* ARM clock, measured, Hz            */
#define S_NEWTOS_CPUPOLICY  2004 /* 0 firmware, 1 max, 2 max refused   */
#define S_NEWTOS_MEMPROT    2005 /* 0 off, 1 on, 2 strict              */
#define S_NEWTOS_ASID       2006 /* 0 off, 1 on, 2 test                */
#define S_NEWTOS_SDMODE     2007 /* 0 fast, 1 compatibility            */
#define S_NEWTOS_SDBUS      2008 /* bus width, bits                    */
#define S_NEWTOS_BCACHE     2009 /* block cache, KiB (0 = off)         */
#define S_NEWTOS_BCACHEPCT  2010 /* linear-read share, percent         */
#define S_NEWTOS_STRAM      2011 /* ST-RAM pool, KiB                   */
#define S_NEWTOS_TTRAM      2012 /* TT-RAM pool, KiB (0 = none)        */
#define S_NEWTOS_SCRWIDTH   2013 /* framebuffer width, px              */
#define S_NEWTOS_SCRHEIGHT  2014 /* framebuffer height, px             */
#define S_NEWTOS_SCRDEPTH   2015 /* framebuffer bits per pixel         */
#define S_NEWTOS_GPUMEM     2016 /* VideoCore memory, KiB              */
#define S_NEWTOS_KEYBOARD   2017 /* 0 US, 1 German, 2 file, 3 changed  */
#define S_NEWTOS_SERIAL     2018 /* 0 off, 1 on                        */
#define S_NEWTOS_KVERSION   2019 /* version string into a buffer       */

/* The kernel delivers the pointer form with one 32-bit store, so the target
 * is an `int` (32 bit under our LP64 app ABI), never a `long`. Returns 1 and
 * fills *out when the kernel knows the value, 0 otherwise. */
static int sys_value(NLONG mode, int *out)
{
    return gemdos3(SSYSTEM, mode, (NLONG)out, 0) == E_OK;
}

/* ------------------------------------------------------------------------- */
/* AES parameter block (the classic layout, LP64 pointers) -- the same shape  */
/* every native NewTOS GEM program uses.                                      */
/* ------------------------------------------------------------------------- */

typedef struct {
    short *control;
    short *global;
    short *intin;
    short *intout;
    long  *addrin;
    long  *addrout;
} AESPB;

static short aes_control[6];
static short aes_global[16];
static short aes_intin[16];
static short aes_intout[16];
static long  aes_addrin[4];
static long  aes_addrout[4];

static void aes_trap(void *pb)
{
    register long x0 __asm__("x0") = 200;
    register long x1 __asm__("x1") = (long)pb;
    __asm__ __volatile__("svc\t#2" : "+r"(x0) : "r"(x1) : "memory", "cc");
    (void)x0;
}

static short aes_call(short opcode, short nin, short nout, short naddrin)
{
    AESPB pb;
    int i;
    aes_control[0] = opcode;
    aes_control[1] = nin;
    aes_control[2] = nout;
    aes_control[3] = naddrin;
    aes_control[4] = 0;
    for (i = 0; i < 16; i++)
        aes_intout[i] = 0;
    pb.control = aes_control;
    pb.global  = aes_global;
    pb.intin   = aes_intin;
    pb.intout  = aes_intout;
    pb.addrin  = aes_addrin;
    pb.addrout = aes_addrout;
    aes_trap(&pb);
    return aes_intout[0];
}

/* AES message types (references/emutos/include/aesdefs.h:242-243). */
#define AC_OPEN   40
#define AC_CLOSE  41

/* form_dial / wind_update codes (EmuTOS include/aesdefs.h). */
#define FMD_START    0
#define FMD_FINISH   3
#define END_UPDATE   0
#define BEG_UPDATE   1
#define END_MCTRL    2
#define BEG_MCTRL    3

/* evnt_multi flags (EmuTOS include/aesdefs.h). */
#define MU_KEYBD   0x0001
#define MU_BUTTON  0x0002
#define MU_M1      0x0004

/* Keys: evnt_multi's kreturn = scan code << 8 | ASCII. */
#define K_ESC      0x1B
#define K_RETURN   0x0D
#define SC_UP      0x48
#define SC_DOWN    0x50

/* ------------------------------------------------------------------------- */
/* Object trees.                                                              */
/* ------------------------------------------------------------------------- */

/* The 32-byte native (LP64) GEM OBJECT: ob_spec is 8 bytes at offset 16
 * (tools/mkrsc.py, tos-api/src/aes/object.rs). */
typedef struct {
    short          ob_next, ob_head, ob_tail;
    unsigned short ob_type, ob_flags, ob_state;
    unsigned long  ob_spec;
    short          ob_x, ob_y, ob_width, ob_height;
} OBJECT;

/* Object types / flags / states (EmuTOS include/aesdefs.h, form.rs). */
#define G_BOX      20
#define G_IBOX     25
#define G_BUTTON   26
#define G_STRING   28

#define SELECTABLE 0x0001
#define DEFAULT    0x0002
#define EXIT       0x0004
#define LASTOB     0x0020
#define FL3DBAK    0x0400
#define FL3DACT    0x0600

#define SELECTED   0x0001
#define CHECKED    0x0004
#define OUTLINED   0x0010
#define SHADOWED   0x0020

/* Box ob_spec: thickness in bits 16..23 (signed), then the colour word
 * (frame colour 1 = black in bits 12..15, text colour 1 in 8..11). The root
 * carries form_alert's spec, 1 << 16; a group frame and the drop-down box
 * are -1 (one pixel) in black, hollow. */
#define ROOT_SPEC  (1UL << 16)
#define FRAME_SPEC (0x00FF1100UL)

/* Geometry, pixels (the 8x16 system font). */
#define CW 8
#define CH 16
#define LABEL_CHARS 12
#define VALUE_CHARS 21
#define VALUE_LEN   (VALUE_CHARS + 1)
#define PAD 8                                   /* inside a group frame */
#define GROUP_W (PAD + LABEL_CHARS * CW + PAD + VALUE_CHARS * CW + PAD) /* 288 */
#define MARGIN 12
#define DIALOG_W (MARGIN + GROUP_W + MARGIN + GROUP_W + MARGIN)          /* 612 */
#define ROW 16

/* The settings group: label | now | next-start drop-down button. */
#define SET_W      (DIALOG_W - 2 * MARGIN)                               /* 588 */
#define SLABEL_X   PAD
#define SLABEL_W   (17 * CW)
#define SNOW_X     (SLABEL_X + SLABEL_W + PAD)                           /* 152 */
#define SNOW_W     (22 * CW)
#define SBTN_X     (SNOW_X + SNOW_W + 3 * PAD)                           /* 352 */
#define BTN_CHARS  27
#define SBTN_W     (BTN_CHARS * CW)                                      /* 216 */
#define SROW       22             /* a 3D button is drawn 2 px larger all round */
#define SROW0      (PAD + ROW + 6)

#define MAX_OBJ 96

static OBJECT tree[MAX_OBJ];
static int nobj;

/* Append an object under `parent` (-1 = the root) of tree `t` (with `*n`
 * objects) and link it as the parent's last child: ob_next of a last child
 * points back at the parent, the classic GEM tree linkage. Coordinates are
 * relative to the parent. */
static short add_to(OBJECT *t, int *n, short parent, unsigned short type,
                    unsigned short flags, unsigned short state,
                    unsigned long spec, short x, short y, short w, short h)
{
    short idx = (short)(*n)++;
    OBJECT *o = &t[idx];

    o->ob_next = -1;
    o->ob_head = -1;
    o->ob_tail = -1;
    o->ob_type = type;
    o->ob_flags = flags;
    o->ob_state = state;
    o->ob_spec = spec;
    o->ob_x = x;
    o->ob_y = y;
    o->ob_width = w;
    o->ob_height = h;

    if (parent >= 0) {
        OBJECT *p = &t[parent];
        if (p->ob_head < 0)
            p->ob_head = idx;
        else
            t[p->ob_tail].ob_next = idx;
        p->ob_tail = idx;
        o->ob_next = parent;
    }
    return idx;
}

static short add(short parent, unsigned short type, unsigned short flags,
                 unsigned short state, unsigned long spec,
                 short x, short y, short w, short h)
{
    return add_to(tree, &nobj, parent, type, flags, state, spec, x, y, w, h);
}

/* ------------------------------------------------------------------------- */
/* AES helpers.                                                               */
/* ------------------------------------------------------------------------- */

static void objc_draw_clip(OBJECT *t, short start, short x, short y,
                           short w, short h)
{
    aes_intin[0] = start;
    aes_intin[1] = 8; /* depth */
    aes_intin[2] = x; aes_intin[3] = y; aes_intin[4] = w; aes_intin[5] = h;
    aes_addrin[0] = (long)t;
    aes_call(42, 6, 1, 1);
}

/* Absolute screen position of object `obj`. */
static void objc_abs(OBJECT *t, short obj, short *x, short *y)
{
    aes_intin[0] = obj;
    aes_addrin[0] = (long)t;
    aes_call(44, 1, 3, 1);
    *x = aes_intout[1];
    *y = aes_intout[2];
}

static short objc_find_at(OBJECT *t, short start, short depth, short mx, short my)
{
    aes_intin[0] = start;
    aes_intin[1] = depth;
    aes_intin[2] = mx;
    aes_intin[3] = my;
    aes_addrin[0] = (long)t;
    return aes_call(43, 4, 1, 1);
}

static void objc_change_to(OBJECT *t, short obj, short cx, short cy, short cw,
                           short ch, unsigned short state)
{
    aes_intin[0] = obj;
    aes_intin[1] = 0;
    aes_intin[2] = cx; aes_intin[3] = cy; aes_intin[4] = cw; aes_intin[5] = ch;
    aes_intin[6] = (short)state;
    aes_intin[7] = 1; /* redraw */
    aes_addrin[0] = (long)t;
    aes_call(47, 8, 1, 1);
}

static void wind_update(short mode)
{
    aes_intin[0] = mode;
    aes_call(107, 1, 1, 0);
}

/* Wait until no mouse button is down (EmuTOS mn_popup's
 * `ev_button(1, 0x00ff, 0x0000, rets)`; returns at once when already up). */
static void wait_button_up(void)
{
    aes_intin[0] = 1;
    aes_intin[1] = 0x00FF;
    aes_intin[2] = 0;
    aes_call(21, 3, 5, 0);
}

static short form_alert(short def, const char *text)
{
    aes_intin[0] = def;
    aes_addrin[0] = (long)text;
    return aes_call(52, 1, 1, 1);
}

/* ------------------------------------------------------------------------- */
/* Text building -- no libc in a freestanding native program.                 */
/* ------------------------------------------------------------------------- */

/* Append `s` to dst[j..] within a buffer of `cap` bytes; returns the new
 * length. dst stays NUL-terminated. */
static int sappn(char *dst, int cap, int j, const char *s)
{
    while (*s && j < cap - 1)
        dst[j++] = *s++;
    dst[j] = '\0';
    return j;
}

static int napps(char *dst, int cap, int j, long v)
{
    char tmp[24];
    int i = 0;
    unsigned long u;

    if (v < 0) {
        j = sappn(dst, cap, j, "-");
        u = (unsigned long)(-v);
    } else {
        u = (unsigned long)v;
    }
    do {
        tmp[i++] = (char)('0' + (int)(u % 10));
        u /= 10;
    } while (u);
    while (i && j < cap - 1)
        dst[j++] = tmp[--i];
    dst[j] = '\0';
    return j;
}

#define sapp(d, j, s) sappn((d), VALUE_LEN, (j), (s))
#define napp(d, j, v) napps((d), VALUE_LEN, (j), (v))

/* `v / div` with one decimal, rounding down -- no floating point here. */
static int fix1app(char *dst, int j, long v, long div)
{
    char d[2];
    long rest;

    if (v < 0) {
        j = sapp(dst, j, "-");
        v = -v;
    }
    rest = v % div;
    j = napp(dst, j, v / div);
    d[0] = (char)('0' + (int)((rest / (div / 10)) % 10));
    d[1] = '\0';
    j = sapp(dst, j, ".");
    return sapp(dst, j, d);
}

/* A size given in KiB as "N MB" (whole MiB, rounded down), or "N KB" below
 * one MiB -- or, with `exact`, "N KB" whenever it is not whole MiB (a
 * block-cache size from the file is shown as it will be used). */
static void size_textn(char *dst, int cap, long kib, int exact)
{
    int j;
    if (kib >= 1024 && (!exact || kib % 1024 == 0)) {
        j = napps(dst, cap, 0, kib / 1024);
        sappn(dst, cap, j, " MB");
    } else {
        j = napps(dst, cap, 0, kib);
        sappn(dst, cap, j, " KB");
    }
}

static const char NA[] = "not available";

/* ------------------------------------------------------------------------- */
/* The read-only rows.                                                        */
/* ------------------------------------------------------------------------- */

enum {
    V_CLOCK, V_MEASURED, V_TEMP, V_STRAM, V_TTRAM,
    V_RES, V_DEPTH, V_GPUMEM, V_SDBUS, V_KEYBOARD,
    V_COUNT
};

static char value[V_COUNT][VALUE_LEN];
static char kernel_text[48];

/* ------------------------------------------------------------------------- */
/* The settings.                                                              */
/* ------------------------------------------------------------------------- */

#define MAX_CHOICES 5

typedef struct {
    const char *label;              /* row label and alert wording */
    int nchoices;
    const char *choice[MAX_CHOICES];
    long val[MAX_CHOICES];          /* cl_settings value of each choice */
} setting_def;

/* Order = CL_* (sysacc-cmdline.h). Not const: a table of pointers in the
 * read-only segment would need relocations there (-z text). */
static setting_def defs[CL_COUNT] = {
    { "Memory protection", 3, { "Off", "On", "Strict" }, { 0, 1, 2 } },
    { "Address-space IDs", 3, { "Off", "On", "Test (rollover)" }, { 0, 1, 2 } },
    { "SD card mode", 2, { "Fast (multi-block)", "Compatibility" }, { 0, 1 } },
    { "CPU clock", 2, { "Fixed at maximum", "Firmware-controlled" }, { 1, 0 } },
    { "Block cache", 5, { "Off", "8 MB", "16 MB", "32 MB", "64 MB" },
      { 0, 8192, 16384, 32768, 65536 } },
    { "Cache share", 4, { "5 %", "10 %", "25 %", "50 %" }, { 5, 10, 25, 50 } },
    { "Mouse while busy", 2, { "On", "Off" }, { 1, 0 } },
    { "Serial console", 2, { "On", "Off" }, { 1, 0 } },
};

static char now_text[CL_COUNT][VALUE_LEN];
static char btn_text[CL_COUNT][BTN_CHARS + 1];
static short btn_obj[CL_COUNT];

static cl_settings file_set;   /* what C:\CMDLINE.TXT says now         */
static cl_settings sel_set;    /* what the dialog has selected         */

/* The file as read when the dialog opened (or as last saved). */
#define FILE_CAP 4096
static char file_buf[FILE_CAP];
static int file_len;
enum { FILE_MISSING, FILE_OK, FILE_BAD };
static int file_state;
static long file_err;

static const char path_txt[] = "C:\\CMDLINE.TXT";
static const char path_bak[] = "C:\\CMDLINE.BAK";
static const char path_new[] = "C:\\CMDLINE.NEW";

static char status_text[80];
static short status_obj, save_button, cancel_button, ok_button;

/* Index of value `v` among setting k's choices, or -1 (a value the file
 * holds that the list does not offer, e.g. bcache=12288). */
static int choice_index(int k, long v)
{
    int i;
    for (i = 0; i < defs[k].nchoices; i++)
        if (defs[k].val[i] == v)
            return i;
    return -1;
}

/* Plain-language text of value `v` of setting k. */
static void value_textn(char *dst, int cap, int k, long v)
{
    int i = choice_index(k, v);
    int j;
    if (i >= 0) {
        sappn(dst, cap, 0, defs[k].choice[i]);
    } else if (k == CL_BCACHE) {
        if (v == 0)
            sappn(dst, cap, 0, "Off");
        else
            size_textn(dst, cap, v, 1);
    } else if (k == CL_BCACHEPCT) {
        j = napps(dst, cap, 0, v);
        sappn(dst, cap, j, " %");
    } else {
        napps(dst, cap, 0, v);
    }
}

/* The drop-down button's text: the value, padded, and the down arrow of
 * the Atari character set (0x02) at the right edge. */
static void set_btn_text(int k)
{
    char *b = btn_text[k];
    int j;
    value_textn(b, BTN_CHARS + 1, k, sel_set.v[k]);
    j = 0;
    while (b[j])
        j++;
    while (j < BTN_CHARS - 3)
        b[j++] = ' ';
    b[j++] = ' ';
    b[j++] = '\x02';
    b[j] = '\0';
}

/* Ask the kernel for every value; called on each AC_OPEN, so the clock and
 * the temperature are the readings of that moment. */
static void read_values(void)
{
    int v, w, h;
    int j;

    /* Kernel version: Ssystem 2019 copies at most len-1 characters + NUL. */
    if (gemdos3(SSYSTEM, S_NEWTOS_KVERSION, (NLONG)kernel_text,
                (NLONG)sizeof(kernel_text)) != E_OK)
        sappn(kernel_text, (int)sizeof(kernel_text), 0, NA);

    /* CPU clock: the rate the firmware was asked for (2000) and the rate it
     * measured (2003). */
    if (sys_value(S_NEWTOS_CPUCLOCK, &v)) {
        j = fix1app(value[V_CLOCK], 0, v, 1000000L);
        sapp(value[V_CLOCK], j, " MHz requested");
    } else {
        sapp(value[V_CLOCK], 0, NA);
    }
    if (sys_value(S_NEWTOS_CPUMEAS, &v)) {
        j = fix1app(value[V_MEASURED], 0, v, 1000000L);
        sapp(value[V_MEASURED], j, " MHz");
    } else {
        sapp(value[V_MEASURED], 0, NA);
    }

    /* Temperature, 1/1000 degree C; 0xF8 is the degree sign of the Atari
     * character set. */
    if (sys_value(S_NEWTOS_CPUTEMP, &v)) {
        j = fix1app(value[V_TEMP], 0, v, 1000L);
        sapp(value[V_TEMP], j, " \xF8""C");
    } else {
        sapp(value[V_TEMP], 0, NA);
    }

    if (sys_value(S_NEWTOS_STRAM, &v))
        size_textn(value[V_STRAM], VALUE_LEN, v, 0);
    else
        sapp(value[V_STRAM], 0, NA);
    if (sys_value(S_NEWTOS_TTRAM, &v)) {
        if (v == 0)
            sapp(value[V_TTRAM], 0, "none");
        else
            size_textn(value[V_TTRAM], VALUE_LEN, v, 0);
    } else {
        sapp(value[V_TTRAM], 0, NA);
    }

    /* Display: the framebuffer the kernel runs. */
    if (sys_value(S_NEWTOS_SCRWIDTH, &w) && sys_value(S_NEWTOS_SCRHEIGHT, &h)) {
        j = napp(value[V_RES], 0, w);
        j = sapp(value[V_RES], j, " x ");
        napp(value[V_RES], j, h);
    } else {
        sapp(value[V_RES], 0, NA);
    }
    if (sys_value(S_NEWTOS_SCRDEPTH, &v)) {
        j = napp(value[V_DEPTH], 0, v);
        sapp(value[V_DEPTH], j, v >= 24 ? "-bit truecolour" : "-bit");
    } else {
        sapp(value[V_DEPTH], 0, NA);
    }
    if (sys_value(S_NEWTOS_GPUMEM, &v))
        size_textn(value[V_GPUMEM], VALUE_LEN, v, 0);
    else
        sapp(value[V_GPUMEM], 0, NA);

    if (sys_value(S_NEWTOS_SDBUS, &v)) {
        j = napp(value[V_SDBUS], 0, v);
        sapp(value[V_SDBUS], j, "-bit");
    } else {
        sapp(value[V_SDBUS], 0, NA);
    }
    if (sys_value(S_NEWTOS_KEYBOARD, &v))
        sapp(value[V_KEYBOARD], 0,
             v == 0 ? "US (built-in)" :
             v == 1 ? "German (built-in)" :
             v == 2 ? "KEYBOARD.KBD" : "Changed by program");
    else
        sapp(value[V_KEYBOARD], 0, NA);

    /* The settings as the kernel runs them now. */
    if (sys_value(S_NEWTOS_MEMPROT, &v))
        value_textn(now_text[CL_MEMPROT], VALUE_LEN, CL_MEMPROT, v);
    else
        sapp(now_text[CL_MEMPROT], 0, NA);
    if (sys_value(S_NEWTOS_ASID, &v))
        value_textn(now_text[CL_ASID], VALUE_LEN, CL_ASID, v);
    else
        sapp(now_text[CL_ASID], 0, NA);
    if (sys_value(S_NEWTOS_SDMODE, &v))
        value_textn(now_text[CL_SDMODE], VALUE_LEN, CL_SDMODE, v ? 1 : 0);
    else
        sapp(now_text[CL_SDMODE], 0, NA);
    /* 2004: 0 firmware, 1 fixed max, 2 = the maximum was asked for but the
     * firmware refused it -- not a choice, a state of its own. */
    if (sys_value(S_NEWTOS_CPUPOLICY, &v))
        sapp(now_text[CL_CPUCLK], 0,
             v == 1 ? "Fixed at maximum" :
             v == 2 ? "Firmware (max denied)" : "Firmware-controlled");
    else
        sapp(now_text[CL_CPUCLK], 0, NA);
    if (sys_value(S_NEWTOS_BCACHE, &v))
        value_textn(now_text[CL_BCACHE], VALUE_LEN, CL_BCACHE, v);
    else
        sapp(now_text[CL_BCACHE], 0, NA);
    if (sys_value(S_NEWTOS_BCACHEPCT, &v))
        value_textn(now_text[CL_BCACHEPCT], VALUE_LEN, CL_BCACHEPCT, v);
    else
        /* No cache, no share: say so rather than a percentage of nothing. */
        sapp(now_text[CL_BCACHEPCT], 0, "-");
    /* No Ssystem mode reports the input mode (input=aes) yet. */
    sapp(now_text[CL_INPUT], 0, NA);
    if (sys_value(S_NEWTOS_SERIAL, &v))
        value_textn(now_text[CL_SERIAL], VALUE_LEN, CL_SERIAL, v ? 1 : 0);
    else
        sapp(now_text[CL_SERIAL], 0, NA);
}

/* Read C:\CMDLINE.TXT into file_buf and parse it into file_set. */
static void read_file(void)
{
    long h, n;

    file_len = 0;
    file_err = 0;
    h = Fopen(path_txt, 0);
    if (h < 0) {
        if (h == EFILNF) {
            file_state = FILE_MISSING;
        } else {
            file_state = FILE_BAD;
            file_err = h;
        }
    } else {
        n = Fread(h, FILE_CAP, file_buf);
        Fclose(h);
        if (n < 0) {
            file_state = FILE_BAD;
            file_err = n;
        } else if (n >= FILE_CAP) {
            /* 4 KB or more: the accessory could not keep all of it. */
            file_state = FILE_BAD;
        } else {
            file_state = FILE_OK;
            file_len = (int)n;
        }
    }
    cl_parse(file_state == FILE_OK ? file_buf : 0, file_len, &file_set);
}

static void copy_settings(cl_settings *dst, const cl_settings *src)
{
    int k;
    for (k = 0; k < CL_COUNT; k++)
        dst->v[k] = src->v[k];
}

static int changes(void)
{
    int k, n = 0;
    for (k = 0; k < CL_COUNT; k++)
        if (sel_set.v[k] != file_set.v[k])
            n++;
    return n;
}

static void set_status(const char *a, long num, const char *b)
{
    int j = sappn(status_text, (int)sizeof(status_text), 0, a);
    if (num) {
        j = napps(status_text, (int)sizeof(status_text), j, num);
        sappn(status_text, (int)sizeof(status_text), j, b);
    }
}

static void initial_status(void)
{
    if (file_state == FILE_OK)
        set_status("Next start: as selected above, from C:\\CMDLINE.TXT.", 0, 0);
    else if (file_state == FILE_MISSING)
        set_status("No C:\\CMDLINE.TXT yet: the next start uses the defaults.", 0, 0);
    else if (file_err)
        set_status("C:\\CMDLINE.TXT cannot be read (error ", file_err,
                   "): saving is off.");
    else
        set_status("C:\\CMDLINE.TXT is 4 KB or larger: saving is off.", 0, 0);
}

/* ------------------------------------------------------------------------- */
/* The tree.                                                                  */
/* ------------------------------------------------------------------------- */

/* One "label  value" row inside an info group, row index `r` below its
 * heading. */
static void row(short group, int r, const char *label, int v)
{
    short y = (short)(PAD + ROW + r * ROW);
    add(group, G_STRING, 0, 0, (unsigned long)label,
        PAD, y, LABEL_CHARS * CW, CH);
    add(group, G_STRING, 0, 0, (unsigned long)value[v],
        PAD + LABEL_CHARS * CW + PAD, y, VALUE_CHARS * CW, CH);
}

/* A framed group of `rows` rows with a heading line; returns its index. */
static short group(short root, short x, short y, int rows, const char *title)
{
    short g = add(root, G_IBOX, 0, 0, FRAME_SPEC, x, y, GROUP_W,
                  (short)(PAD + ROW + rows * ROW + PAD));
    add(g, G_STRING, 0, 0, (unsigned long)title, PAD, PAD - 4,
        (short)(GROUP_W - 2 * PAD), CH);
    return g;
}

static void build_tree(void)
{
    short root, g, s;
    short top, y;
    int k;

    nobj = 0;
    root = add(-1, G_BOX, FL3DBAK, OUTLINED, ROOT_SPEC, 0, 0, DIALOG_W, 0);

    add(root, G_STRING, 0, 0, (unsigned long)"NewTOS System Information",
        (short)((DIALOG_W - 25 * CW) / 2), 6, 25 * CW, CH);
    add(root, G_STRING, 0, 0, (unsigned long)"Kernel",
        MARGIN + PAD, 26, LABEL_CHARS * CW, CH);
    add(root, G_STRING, 0, 0, (unsigned long)kernel_text,
        MARGIN + PAD + LABEL_CHARS * CW + PAD, 26, 47 * CW, CH);

    top = 48;
    g = group(root, MARGIN, top, 5, "System");
    row(g, 0, "Clock rate", V_CLOCK);
    row(g, 1, "Measured", V_MEASURED);
    row(g, 2, "Temperature", V_TEMP);
    row(g, 3, "ST-RAM", V_STRAM);
    row(g, 4, "TT-RAM", V_TTRAM);
    g = group(root, MARGIN + GROUP_W + MARGIN, top, 5, "Display and devices");
    row(g, 0, "Resolution", V_RES);
    row(g, 1, "Colour depth", V_DEPTH);
    row(g, 2, "GPU memory", V_GPUMEM);
    row(g, 3, "SD bus width", V_SDBUS);
    row(g, 4, "Keyboard", V_KEYBOARD);
    y = (short)(top + tree[g].ob_height + 8);

    /* The settings: label | running now | next start (drop-down). */
    s = add(root, G_IBOX, 0, 0, FRAME_SPEC, MARGIN, y, SET_W,
            (short)(SROW0 + CL_COUNT * SROW + 4));
    add(s, G_STRING, 0, 0, (unsigned long)"Settings",
        SLABEL_X, PAD - 4, SLABEL_W + PAD, CH);
    add(s, G_STRING, 0, 0, (unsigned long)"Running now",
        SNOW_X, PAD - 4, SNOW_W, CH);
    add(s, G_STRING, 0, 0, (unsigned long)"Next start",
        SBTN_X, PAD - 4, SBTN_W, CH);
    for (k = 0; k < CL_COUNT; k++) {
        short ry = (short)(SROW0 + k * SROW);
        set_btn_text(k);
        add(s, G_STRING, 0, 0, (unsigned long)defs[k].label,
            SLABEL_X, ry, SLABEL_W, CH);
        add(s, G_STRING, 0, 0, (unsigned long)now_text[k],
            SNOW_X, ry, SNOW_W, CH);
        btn_obj[k] = add(s, G_BUTTON, SELECTABLE | EXIT | FL3DACT, 0,
                         (unsigned long)btn_text[k], SBTN_X, ry, SBTN_W, CH);
    }
    y = (short)(y + tree[s].ob_height + 6);

    add(root, G_STRING, 0, 0,
        (unsigned long)"Changes take effect after restart.",
        MARGIN + PAD, y, 34 * CW, CH);
    y = (short)(y + ROW);
    status_obj = add(root, G_STRING, 0, 0, (unsigned long)status_text,
                     MARGIN + PAD, y, (short)(SET_W - 2 * PAD), CH);
    y = (short)(y + ROW + 8);

    save_button = add(root, G_BUTTON, SELECTABLE | EXIT | FL3DACT, 0,
                      (unsigned long)"Save", MARGIN + PAD, y, 10 * CW, CH + 2);
    cancel_button = add(root, G_BUTTON, SELECTABLE | EXIT | FL3DACT, 0,
                        (unsigned long)"Cancel",
                        (short)(DIALOG_W - MARGIN - PAD - 22 * CW), y,
                        10 * CW, CH + 2);
    ok_button = add(root, G_BUTTON, SELECTABLE | DEFAULT | EXIT | FL3DACT, 0,
                    (unsigned long)"OK",
                    (short)(DIALOG_W - MARGIN - PAD - 10 * CW), y,
                    10 * CW, CH + 2);
    tree[root].ob_height = (short)(y + CH + 2 + 10);
    tree[nobj - 1].ob_flags |= LASTOB;
}

/* Redraw the dialog inside the rectangle around object `obj` (with `m`
 * pixels of margin for 3D edges): drawing from the root, clipped, so every
 * layer under the object comes back as well. */
static void redraw_obj(short obj, short m)
{
    short x, y;
    objc_abs(tree, obj, &x, &y);
    objc_draw_clip(tree, 0, (short)(x - m), (short)(y - m),
                   (short)(tree[obj].ob_width + 2 * m),
                   (short)(tree[obj].ob_height + 2 * m));
}

/* ------------------------------------------------------------------------- */
/* The drop-down.                                                             */
/* ------------------------------------------------------------------------- */

static OBJECT pop[MAX_CHOICES + 1];
static char pop_text[MAX_CHOICES][BTN_CHARS + 1];

/* Run the drop-down of setting k over its button; `dx..dh` is the dialog's
 * rectangle the box must stay inside. Returns the chosen choice index or -1
 * (cancelled). */
static int popup(int k, short dx, short dy, short dw, short dh)
{
    int n = 0, i, cur, nch = defs[k].nchoices;
    short bx, by, px, py, pw, ph, cx, cy, cw, chh;
    short mx, my, obj, oldobj, events, key;
    short m1out, m1x, m1y, m1w, m1h;
    short msg[8];
    int result = -1;

    wind_update(BEG_MCTRL);   /* take over the mouse */
    wait_button_up();

    cur = choice_index(k, sel_set.v[k]);
    objc_abs(tree, btn_obj[k], &bx, &by);
    pw = SBTN_W;
    ph = (short)(nch * CH);
    px = bx;
    /* The current choice sits over the button, as mn_popup places it. */
    py = (short)(by - (cur > 0 ? cur : 0) * CH);
    if (py < dy + 8)
        py = (short)(dy + 8);
    if (py + ph > dy + dh - 12)
        py = (short)(dy + dh - 12 - ph);
    if (px + pw > dx + dw - 12)
        px = (short)(dx + dw - 12 - pw);

    add_to(pop, &n, -1, G_BOX, FL3DBAK, SHADOWED, FRAME_SPEC, px, py, pw, ph);
    for (i = 0; i < nch; i++) {
        int j = sappn(pop_text[i], BTN_CHARS + 1, 0, "  ");
        sappn(pop_text[i], BTN_CHARS + 1, j, defs[k].choice[i]);
        add_to(pop, &n, 0, G_STRING, SELECTABLE, i == cur ? CHECKED : 0,
               (unsigned long)pop_text[i], 0, (short)(i * CH), pw, CH);
    }
    pop[n - 1].ob_flags |= LASTOB;

    /* The area the box covers, 3D edge, frame and shadow included. */
    cx = (short)(px - 6);
    cy = (short)(py - 6);
    cw = (short)(pw + 14);
    chh = (short)(ph + 14);

    /* Like handle_popup: the start item selected, then the whole box. */
    oldobj = (short)(cur >= 0 ? cur + 1 : 1);
    pop[oldobj].ob_state |= SELECTED;
    objc_draw_clip(pop, 0, cx, cy, cw, chh);

    aes_call(79, 0, 5, 0); /* graf_mkstate */
    mx = aes_intout[1];
    my = aes_intout[2];

    for (;;) {
        /* M1: leave the object under the pointer, or enter the box. */
        obj = objc_find_at(pop, 0, 1, mx, my);
        if (obj < 0) {
            m1out = 0;
            m1x = px; m1y = py; m1w = pw; m1h = ph;
        } else {
            m1out = 1;
            objc_abs(pop, obj, &m1x, &m1y);
            m1w = pop[obj].ob_width;
            m1h = pop[obj].ob_height;
        }
        for (i = 0; i < 16; i++)
            aes_intin[i] = 0;
        aes_intin[0] = MU_KEYBD | MU_BUTTON | MU_M1;
        aes_intin[1] = 1;      /* one click      */
        aes_intin[2] = 1;      /* left button    */
        aes_intin[3] = 1;      /* ... down       */
        aes_intin[4] = m1out;
        aes_intin[5] = m1x; aes_intin[6] = m1y;
        aes_intin[7] = m1w; aes_intin[8] = m1h;
        aes_addrin[0] = (long)msg;
        events = aes_call(25, 16, 7, 1); /* evnt_multi */
        mx = aes_intout[1];
        my = aes_intout[2];
        key = aes_intout[5];

        obj = objc_find_at(pop, 0, 1, mx, my);

        if (events & MU_KEYBD) {
            int asc = key & 0xFF, sc = (key >> 8) & 0xFF;
            short next = oldobj;
            if (asc == K_ESC || sc == 0x01) {
                result = -1;
                break;
            }
            if (asc == K_RETURN) {
                result = oldobj >= 1 ? oldobj - 1 : -1;
                break;
            }
            if (sc == SC_UP && oldobj > 1)
                next = (short)(oldobj - 1);
            else if (sc == SC_DOWN && oldobj < nch)
                next = (short)(oldobj + 1);
            if (next != oldobj) {
                objc_change_to(pop, oldobj, cx, cy, cw, chh,
                               (unsigned short)(pop[oldobj].ob_state & ~SELECTED));
                objc_change_to(pop, next, cx, cy, cw, chh,
                               (unsigned short)(pop[next].ob_state | SELECTED));
                oldobj = next;
            }
            continue;
        }

        if (events & MU_M1) {
            /* The pointer moved: the item under it is highlighted. */
            if (obj >= 1 && obj != oldobj) {
                if (oldobj >= 1)
                    objc_change_to(pop, oldobj, cx, cy, cw, chh,
                                   (unsigned short)(pop[oldobj].ob_state & ~SELECTED));
                objc_change_to(pop, obj, cx, cy, cw, chh,
                               (unsigned short)(pop[obj].ob_state | SELECTED));
                oldobj = obj;
            }
        }

        if (events & MU_BUTTON) {
            /* A click on an item chooses it, anywhere else cancels. */
            result = obj >= 1 ? obj - 1 : -1;
            break;
        }
    }

    /* The box lies inside the dialog: the dialog redraws what it covered. */
    objc_draw_clip(tree, 0, cx, cy, cw, chh);

    wait_button_up();
    wind_update(END_MCTRL);   /* give back the mouse */
    return result;
}

/* ------------------------------------------------------------------------- */
/* Saving.                                                                    */
/* ------------------------------------------------------------------------- */

/* Write `len` bytes as file `name` (created or truncated). 0 or an error. */
static long write_file(const char *name, const char *buf, int len)
{
    long h = Fcreate(name, 0);
    long w, c;
    if (h < 0)
        return h;
    w = Fwrite(h, len, buf);
    c = Fclose(h);
    if (w < 0)
        return w;
    if (w != len)
        return -1;  /* ERROR: short write (disk full) */
    return c < 0 ? c : 0;
}

static char new_buf[FILE_CAP + 256];
static char alert_buf[320];

/* "[3][<what> failed|(GEMDOS error -N).|<hint>][ OK ]" */
static void fail_alert(const char *what, long err, const char *hint)
{
    int c = (int)sizeof(alert_buf), j;
    j = sappn(alert_buf, c, 0, "[3][");
    j = sappn(alert_buf, c, j, what);
    j = sappn(alert_buf, c, j, "|failed (GEMDOS error ");
    j = napps(alert_buf, c, j, err);
    j = sappn(alert_buf, c, j, ").|");
    j = sappn(alert_buf, c, j, hint);
    sappn(alert_buf, c, j, "][ OK ]");
    form_alert(1, alert_buf);
}

/* Confirm and save. Returns 1 when the file now holds the selection (also
 * when there was nothing to change), 0 when the user cancelled or a step
 * failed. */
static int save(void)
{
    int c = (int)sizeof(alert_buf), j, k, shown = 0, n, len;
    long r;

    n = changes();
    if (n == 0) {
        set_status("Nothing to save: C:\\CMDLINE.TXT already says this.", 0, 0);
        redraw_obj(status_obj, 0);
        return 1;
    }
    if (file_state == FILE_BAD) {
        form_alert(1, "[3][C:\\CMDLINE.TXT cannot be read|completely, so it is not|"
                      "changed. Nothing was saved.][ OK ]");
        return 0;
    }

    /* One confirmation, the changes in plain words (form_alert: five lines
     * of at most 40 characters). */
    j = sappn(alert_buf, c, 0, "[2][For the next start:");
    for (k = 0; k < CL_COUNT; k++) {
        char v[BTN_CHARS + 1];
        if (sel_set.v[k] == file_set.v[k])
            continue;
        if (shown == 3 && n > 4) {
            j = sappn(alert_buf, c, j, "|... and ");
            j = napps(alert_buf, c, j, n - 3);
            j = sappn(alert_buf, c, j, " more");
            break;
        }
        value_textn(v, (int)sizeof(v), k, sel_set.v[k]);
        j = sappn(alert_buf, c, j, "|");
        j = sappn(alert_buf, c, j, defs[k].label);
        j = sappn(alert_buf, c, j, ": ");
        j = sappn(alert_buf, c, j, v);
        shown++;
    }
    sappn(alert_buf, c, j, "][Save|Cancel]");
    if (form_alert(1, alert_buf) != 1)
        return 0;

    len = cl_merge(file_state == FILE_OK ? file_buf : 0, file_len, &sel_set,
                   new_buf, FILE_CAP - 1);
    if (len < 0) {
        form_alert(1, "[3][The new C:\\CMDLINE.TXT would be|4 KB or larger. "
                      "Nothing was saved.][ OK ]");
        return 0;
    }

    /* 1. the backup of the old file */
    if (file_state == FILE_OK) {
        r = write_file(path_bak, file_buf, file_len);
        if (r < 0) {
            fail_alert("Writing C:\\CMDLINE.BAK", r,
                       "C:\\CMDLINE.TXT is unchanged.");
            return 0;
        }
    }
    /* 2. the new text under a temporary name */
    r = write_file(path_new, new_buf, len);
    if (r < 0) {
        Fdelete(path_new);
        fail_alert("Writing C:\\CMDLINE.NEW", r, "C:\\CMDLINE.TXT is unchanged.");
        return 0;
    }
    /* 3. the old file out of the way (Frename does not replace) */
    if (file_state == FILE_OK) {
        r = Fdelete(path_txt);
        if (r < 0 && r != EFILNF) {
            Fdelete(path_new);
            fail_alert("Deleting C:\\CMDLINE.TXT", r,
                       "C:\\CMDLINE.TXT is unchanged.");
            return 0;
        }
    }
    /* 4. the new file in its place */
    r = Frename(path_new, path_txt);
    if (r < 0) {
        fail_alert("Renaming C:\\CMDLINE.NEW", r,
                   file_state == FILE_OK ? "The old text is in C:\\CMDLINE.BAK."
                                         : "Nothing else was changed.");
        return 0;
    }

    /* The file now holds the selection. */
    for (j = 0; j < len; j++)
        file_buf[j] = new_buf[j];
    file_len = len;
    if (file_state == FILE_OK)
        set_status("Saved to C:\\CMDLINE.TXT, the old one is C:\\CMDLINE.BAK.", 0, 0);
    else
        set_status("Saved to C:\\CMDLINE.TXT.", 0, 0);
    file_state = FILE_OK;
    cl_parse(file_buf, file_len, &file_set);
    redraw_obj(status_obj, 0);
    return 1;
}

/* ------------------------------------------------------------------------- */
/* The dialog.                                                                */
/* ------------------------------------------------------------------------- */

static void show_dialog(void)
{
    short x, y, w, h;
    short exitobj;
    int k, done = 0;

    read_values();
    read_file();
    copy_settings(&sel_set, &file_set);
    initial_status();
    build_tree();

    wind_update(BEG_UPDATE);

    aes_addrin[0] = (long)tree;
    aes_call(54, 0, 5, 1); /* form_center */
    x = aes_intout[1];
    y = aes_intout[2];
    w = aes_intout[3];
    h = aes_intout[4];

    aes_intin[0] = FMD_START;
    aes_intin[1] = x; aes_intin[2] = y; aes_intin[3] = w; aes_intin[4] = h;
    aes_intin[5] = x; aes_intin[6] = y; aes_intin[7] = w; aes_intin[8] = h;
    aes_call(51, 9, 1, 0); /* form_dial */

    objc_draw_clip(tree, 0, x, y, w, h);

    while (!done) {
        aes_intin[0] = 0;  /* no editable field */
        aes_addrin[0] = (long)tree;
        exitobj = (short)(aes_call(50, 1, 1, 1) & 0x7FFF); /* form_do */
        if (exitobj < 0 || exitobj >= nobj)
            break;
        tree[exitobj].ob_state &= (unsigned short)~SELECTED;

        if (exitobj == cancel_button) {
            break;
        } else if (exitobj == ok_button) {
            /* OK keeps the selection: save it first if it is not saved. */
            if (changes() == 0 || save())
                break;
            redraw_obj(exitobj, 4);
        } else if (exitobj == save_button) {
            redraw_obj(exitobj, 4);
            save();
        } else {
            for (k = 0; k < CL_COUNT; k++)
                if (btn_obj[k] == exitobj)
                    break;
            redraw_obj(exitobj, 4);
            if (k < CL_COUNT) {
                int i = popup(k, x, y, w, h);
                if (i >= 0 && defs[k].val[i] != sel_set.v[k]) {
                    sel_set.v[k] = defs[k].val[i];
                    set_btn_text(k);
                    redraw_obj(exitobj, 4);
                }
            }
        }
    }

    aes_intin[0] = FMD_FINISH;
    aes_intin[1] = x; aes_intin[2] = y; aes_intin[3] = w; aes_intin[4] = h;
    aes_intin[5] = x; aes_intin[6] = y; aes_intin[7] = w; aes_intin[8] = h;
    aes_call(51, 9, 1, 0); /* form_dial */

    wind_update(END_UPDATE);
    aes_addrin[0] = 0;
}

/* The accessory's entry in the Desk menu. A leading space is the classic
 * look: the drop-down puts no padding of its own in front of the title. */
static const char acc_title[] = " System Info";

int main(void)
{
    short msg[8];
    short apid, menu_id;
    int i;

    apid = aes_call(10, 0, 1, 0); /* appl_init */
    if (apid < 0)
        return 1;

    aes_intin[0] = apid;
    aes_addrin[0] = (long)acc_title;
    menu_id = aes_call(35, 1, 1, 1); /* menu_register */
    aes_addrin[0] = 0;
    if (menu_id < 0) {
        /* No free accessory slot. Classic accessories simply give up here;
         * there is nothing to clean up and nothing to show. */
        return 1;
    }

    /*
     * An accessory never terminates: it waits for messages for as long as the
     * machine runs.
     */
    for (;;) {
        for (i = 0; i < 8; i++)
            msg[i] = 0;
        aes_addrin[0] = (long)msg;
        aes_call(23, 0, 1, 1); /* evnt_mesag */
        aes_addrin[0] = 0;

        /*
         * AC_OPEN carries the accessory's menu id in msg[4]
         * (EmuTOS gemctrl.c:337-346 -> ap_sendmsg, gemwmlib.c:851-862);
         * AC_CLOSE carries it in msg[3] (gemmnlib.c:747). The asymmetry is
         * EmuTOS's, not a typo here.
         */
        if (msg[0] == AC_OPEN && msg[4] == menu_id) {
            show_dialog();
        } else if (msg[0] == AC_CLOSE && msg[3] == menu_id) {
            /* The application that was running is going away. The dialog is
             * modal and closed before this message can be read, and the
             * accessory owns no window and no allocation -- nothing to close,
             * but it must keep running and keep listening. */
        }
    }
}
