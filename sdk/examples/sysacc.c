/*
 * sysacc.c -- SYSINFO01: the "System Info" desk accessory, version 1.
 *
 * A read-only GEM dialog that shows the state the kernel actually runs with:
 * the machine (CPU clock, temperature, kernel version, ST-RAM / TT-RAM), the
 * memory protection and address-space IDs, the SD card's transfer mode and
 * the block cache, the CPU clock policy, the display, the keyboard layout and
 * the serial console. Every value is asked from the kernel through NewTOS's
 * own Ssystem subfunctions (GEMDOS 0x154, modes 2000-2019,
 * docs/api/gemdos/154-ssystem.md) at the moment the dialog opens -- it is the
 * EFFECTIVE state (what the boot code did with cmdline.txt, what the card
 * negotiated), never the text of a configuration file. A value the kernel
 * does not know is shown as "not available", never as an invented number.
 *
 * History: ACC06 built it as one form_alert with the ARM clock and the die
 * temperature (Ssystem 2000/2001); SYSINFO01 (2026-10-07) turned it into this
 * dialog. Editing comes later: each value is an object of its own in a fixed
 * value column, so a later version can turn it into a selectable control
 * (pop-up button, radio buttons) without moving the layout.
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
 * An accessory never terminates, so there is no appl_exit and no return out
 * of the loop.
 *
 * The dialog is the classic modal sequence (EmuTOS gemfmlib.c / the AES
 * reference): wind_update(BEG_UPDATE), form_center, form_dial(FMD_START),
 * objc_draw, form_do, form_dial(FMD_FINISH), wind_update(END_UPDATE). The
 * object tree is built in code -- the accessory is freestanding (no libc, no
 * rsrc_load), and a tree in its own memory is what form_alert's caller-less
 * kernel tree is not: something the next version can edit in place.
 *
 * Look: the AES 3D style (memory note aes-dialog-3d-look; the same flags
 * form_alert uses, tos-api/src/aes/form.rs): the root is an OUTLINED G_BOX
 * with FL3DBAK (the grey 3D panel), the OK button a DEFAULT|EXIT G_BUTTON
 * with FL3DACT. Groups are framed G_IBOXes. 8x16 system font, the whole
 * dialog is 612 x 324 pixels -- it fits a 640 x 480 screen with its menu bar.
 *
 * Calls used, all against docs/api/ and the EmuTOS AES:
 *   appl_init       AES  10
 *   evnt_mesag      AES  23
 *   menu_register   AES  35
 *   objc_draw       AES  42
 *   form_do         AES  50
 *   form_dial       AES  51
 *   form_center     AES  54
 *   wind_update     AES 107
 *   Ssystem         GEMDOS 0x154, modes 2000-2019
 */
#include <newtos.h>

/* ------------------------------------------------------------------------- */
/* GEMDOS: Ssystem takes three arguments; newtos.h ships gemdos0/1/2 only.    */
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

#define SSYSTEM 0x154
#define E_OK    0L

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

/* ------------------------------------------------------------------------- */
/* The object tree.                                                           */
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
#define OUTLINED   0x0010

/* Box ob_spec: thickness in bits 16..23 (signed), then the colour word
 * (frame colour 1 = black in bits 12..15, text colour 1 in 8..11). The root
 * carries form_alert's spec, 1 << 16; a group frame is -1 (one pixel inside)
 * in black, hollow. */
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

#define MAX_OBJ 64

static OBJECT tree[MAX_OBJ];
static int nobj;
static short ok_button;

/* Append an object under `parent` (-1 = the root) and link it as the
 * parent's last child: ob_next of a last child points back at the parent,
 * the classic GEM tree linkage. Coordinates are relative to the parent. */
static short add(short parent, unsigned short type, unsigned short flags,
                 unsigned short state, unsigned long spec,
                 short x, short y, short w, short h)
{
    short idx = (short)nobj++;
    OBJECT *o = &tree[idx];

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
        OBJECT *p = &tree[parent];
        if (p->ob_head < 0)
            p->ob_head = idx;
        else
            tree[p->ob_tail].ob_next = idx;
        p->ob_tail = idx;
        o->ob_next = parent;
    }
    return idx;
}

/* ------------------------------------------------------------------------- */
/* Text building -- no libc in a freestanding native program.                 */
/* ------------------------------------------------------------------------- */

static int sapp(char *dst, int j, const char *s)
{
    while (*s && j < VALUE_LEN - 1)
        dst[j++] = *s++;
    dst[j] = '\0';
    return j;
}

static int napp(char *dst, int j, long v)
{
    char tmp[24];
    int i = 0;
    unsigned long u;

    if (v < 0) {
        j = sapp(dst, j, "-");
        u = (unsigned long)(-v);
    } else {
        u = (unsigned long)v;
    }
    do {
        tmp[i++] = (char)('0' + (int)(u % 10));
        u /= 10;
    } while (u);
    while (i && j < VALUE_LEN - 1)
        dst[j++] = tmp[--i];
    dst[j] = '\0';
    return j;
}

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

/* A size given in KiB as "N MB" (whole MiB), or "N KB" below one MiB. */
static void size_text(char *dst, long kib)
{
    int j;
    if (kib >= 1024) {
        j = napp(dst, 0, kib / 1024);
        sapp(dst, j, " MB");
    } else {
        j = napp(dst, 0, kib);
        sapp(dst, j, " KB");
    }
}

static const char NA[] = "not available";

/* ------------------------------------------------------------------------- */
/* The rows.                                                                  */
/* ------------------------------------------------------------------------- */

enum {
    V_KERNEL,
    V_CLOCK, V_MEASURED, V_POLICY, V_TEMP, V_STRAM, V_TTRAM,
    V_RES, V_DEPTH, V_GPUMEM,
    V_MEMPROT, V_ASID,
    V_SDMODE, V_SDBUS, V_BCACHE, V_BCACHEPCT,
    V_KEYBOARD, V_SERIAL,
    V_COUNT
};

static char value[V_COUNT][VALUE_LEN];
static char kernel_text[48];

/* Ask the kernel for every value; called on each AC_OPEN, so the clock and
 * the temperature are the readings of that moment. */
static void read_values(void)
{
    int v, w, h;
    int j;

    /* Kernel version: Ssystem 2019 copies at most len-1 characters + NUL. */
    if (gemdos3(SSYSTEM, S_NEWTOS_KVERSION, (NLONG)kernel_text,
                (NLONG)sizeof(kernel_text)) != E_OK)
        sapp(kernel_text, 0, NA);

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
    if (sys_value(S_NEWTOS_CPUPOLICY, &v))
        sapp(value[V_POLICY], 0,
             v == 1 ? "Fixed at maximum" :
             v == 2 ? "Firmware (max denied)" : "Firmware-controlled");
    else
        sapp(value[V_POLICY], 0, NA);

    /* Temperature, 1/1000 degree C; 0xF8 is the degree sign of the Atari
     * character set. */
    if (sys_value(S_NEWTOS_CPUTEMP, &v)) {
        j = fix1app(value[V_TEMP], 0, v, 1000L);
        sapp(value[V_TEMP], j, " \xF8""C");
    } else {
        sapp(value[V_TEMP], 0, NA);
    }

    if (sys_value(S_NEWTOS_STRAM, &v))
        size_text(value[V_STRAM], v);
    else
        sapp(value[V_STRAM], 0, NA);
    if (sys_value(S_NEWTOS_TTRAM, &v)) {
        if (v == 0)
            sapp(value[V_TTRAM], 0, "none");
        else
            size_text(value[V_TTRAM], v);
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
        size_text(value[V_GPUMEM], v);
    else
        sapp(value[V_GPUMEM], 0, NA);

    /* Memory protection and address-space IDs. */
    if (sys_value(S_NEWTOS_MEMPROT, &v))
        sapp(value[V_MEMPROT], 0, v == 2 ? "Strict" : v == 1 ? "On" : "Off");
    else
        sapp(value[V_MEMPROT], 0, NA);
    if (sys_value(S_NEWTOS_ASID, &v))
        sapp(value[V_ASID], 0, v == 2 ? "Test (rollover)" : v == 1 ? "On" : "Off");
    else
        sapp(value[V_ASID], 0, NA);

    /* SD card and block cache. */
    if (sys_value(S_NEWTOS_SDMODE, &v))
        sapp(value[V_SDMODE], 0, v ? "Compatibility" : "Fast (multi-block)");
    else
        sapp(value[V_SDMODE], 0, NA);
    if (sys_value(S_NEWTOS_SDBUS, &v)) {
        j = napp(value[V_SDBUS], 0, v);
        sapp(value[V_SDBUS], j, "-bit");
    } else {
        sapp(value[V_SDBUS], 0, NA);
    }
    if (sys_value(S_NEWTOS_BCACHE, &v)) {
        if (v == 0)
            sapp(value[V_BCACHE], 0, "Off");
        else
            size_text(value[V_BCACHE], v);
    } else {
        sapp(value[V_BCACHE], 0, NA);
    }
    if (sys_value(S_NEWTOS_BCACHEPCT, &v)) {
        j = napp(value[V_BCACHEPCT], 0, v);
        sapp(value[V_BCACHEPCT], j, " %");
    } else {
        /* No cache, no share: say so rather than a percentage of nothing. */
        sapp(value[V_BCACHEPCT], 0, "-");
    }

    /* Keyboard and serial console. */
    if (sys_value(S_NEWTOS_KEYBOARD, &v))
        sapp(value[V_KEYBOARD], 0,
             v == 0 ? "US (built-in)" :
             v == 1 ? "German (built-in)" :
             v == 2 ? "KEYBOARD.KBD" : "Changed by program");
    else
        sapp(value[V_KEYBOARD], 0, NA);
    if (sys_value(S_NEWTOS_SERIAL, &v))
        sapp(value[V_SERIAL], 0, v ? "On" : "Off");
    else
        sapp(value[V_SERIAL], 0, NA);
}

/* One "label  value" row inside a group, row index `r` below its heading.
 * The value is its own object in the value column -- the slot a later
 * version turns into a control. */
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
    short root, g;
    short top, lx, rx, ly, ry;

    nobj = 0;
    root = add(-1, G_BOX, FL3DBAK, OUTLINED, ROOT_SPEC, 0, 0, DIALOG_W, 0);

    add(root, G_STRING, 0, 0, (unsigned long)"NewTOS System Information",
        (short)((DIALOG_W - 25 * CW) / 2), 8, 25 * CW, CH);
    add(root, G_STRING, 0, 0, (unsigned long)"Kernel",
        MARGIN + PAD, 32, LABEL_CHARS * CW, CH);
    add(root, G_STRING, 0, 0, (unsigned long)kernel_text,
        MARGIN + PAD + LABEL_CHARS * CW + PAD, 32, 47 * CW, CH);

    top = 56;
    lx = MARGIN;
    rx = MARGIN + GROUP_W + MARGIN;

    /* Left column: System, Display. */
    ly = top;
    g = group(root, lx, ly, 6, "System");
    row(g, 0, "CPU clock", V_CLOCK);
    row(g, 1, "Measured", V_MEASURED);
    row(g, 2, "Clock policy", V_POLICY);
    row(g, 3, "Temperature", V_TEMP);
    row(g, 4, "ST-RAM", V_STRAM);
    row(g, 5, "TT-RAM", V_TTRAM);
    ly = (short)(ly + tree[g].ob_height + 8);
    g = group(root, lx, ly, 3, "Display");
    row(g, 0, "Resolution", V_RES);
    row(g, 1, "Colour depth", V_DEPTH);
    row(g, 2, "GPU memory", V_GPUMEM);
    ly = (short)(ly + tree[g].ob_height);

    /* Right column: Memory protection, SD card, Input / output. */
    ry = top;
    g = group(root, rx, ry, 2, "Memory protection");
    row(g, 0, "Protection", V_MEMPROT);
    row(g, 1, "Address IDs", V_ASID);
    ry = (short)(ry + tree[g].ob_height + 8);
    g = group(root, rx, ry, 4, "SD card");
    row(g, 0, "Transfer", V_SDMODE);
    row(g, 1, "Bus width", V_SDBUS);
    row(g, 2, "Block cache", V_BCACHE);
    row(g, 3, "Cache share", V_BCACHEPCT);
    ry = (short)(ry + tree[g].ob_height + 8);
    g = group(root, rx, ry, 2, "Input / output");
    row(g, 0, "Keyboard", V_KEYBOARD);
    row(g, 1, "Serial port", V_SERIAL);
    ry = (short)(ry + tree[g].ob_height);

    if (ry > ly)
        ly = ry;
    ok_button = add(root, G_BUTTON, SELECTABLE | DEFAULT | EXIT | FL3DACT, 0,
                    (unsigned long)"OK",
                    (short)((DIALOG_W - 10 * CW) / 2), (short)(ly + 12),
                    10 * CW, CH + 6);
    tree[root].ob_height = (short)(ly + 12 + CH + 6 + 12);
    tree[nobj - 1].ob_flags |= LASTOB;
}

/* Run the dialog once: the classic modal sequence. */
static void show_dialog(void)
{
    short x, y, w, h;
    short exitobj;

    read_values();
    build_tree();

    aes_intin[0] = BEG_UPDATE;
    aes_call(107, 1, 1, 0); /* wind_update */

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

    aes_intin[0] = 0;  /* start at the root */
    aes_intin[1] = 8;  /* depth */
    aes_intin[2] = x; aes_intin[3] = y; aes_intin[4] = w; aes_intin[5] = h;
    aes_addrin[0] = (long)tree;
    aes_call(42, 6, 1, 1); /* objc_draw */

    aes_intin[0] = 0;  /* no editable field */
    aes_addrin[0] = (long)tree;
    exitobj = (short)(aes_call(50, 1, 1, 1) & 0x7FFF); /* form_do */
    if (exitobj >= 0 && exitobj < nobj)
        tree[exitobj].ob_state &= (unsigned short)~SELECTED;

    aes_intin[0] = FMD_FINISH;
    aes_intin[1] = x; aes_intin[2] = y; aes_intin[3] = w; aes_intin[4] = h;
    aes_intin[5] = x; aes_intin[6] = y; aes_intin[7] = w; aes_intin[8] = h;
    aes_call(51, 9, 1, 0); /* form_dial */

    aes_intin[0] = END_UPDATE;
    aes_call(107, 1, 1, 0); /* wind_update */
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
