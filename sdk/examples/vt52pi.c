/*
 * vt52pi.c -- TASK/CON04 real-Pi acceptance screen for the VT52 console at
 * the native column count (1920x1080 with the 8x16 font = 240 columns).
 *
 * A native NewTOS GEM application (.NAP), started by double-click from the
 * desktop (or the #Z autorun line of C:\NEWDESK.INF). Why a GEM program and
 * not a console program (.NCP): test 4 calls the VDI escape vs_curaddress
 * through a workstation handle, and a program gets that handle the normal
 * way only through the AES (appl_init, graf_handle, v_opnvwk); an AUTO
 * program runs before the AES and has none. The text screen is entered the
 * way a GEM program does it (graf_mouse(M_OFF), v_enter_cur = ESC E ESC e
 * CR) and left with v_exit_cur, so the desktop redraws itself afterwards.
 *
 * The geometry is read at run time with vq_chcells -- nothing assumes 80 or
 * 240 columns. Every reference mark (the column ruler, the '#' guides) is
 * written SEQUENTIALLY from column 0, so it does not depend on the
 * addressing under test.
 *
 *   T1  wrap off (initial state, no ESC w sent): a row written past the
 *       right edge keeps overwriting the last cell; the next row is
 *       untouched.
 *   T2  wrap on (ESC v): the same row continues on the next row.
 *   T3  ESC Y to the highest encodable column (byte 32 + 223 = 0xFF).
 *   T4  vs_curaddress (Q6, direct position) to column index 230 and to the
 *       last column.
 *   T5  vs_curaddress beyond the screen clamps to the last column / row;
 *       vq_curaddress reads the clamped position back.
 *   T6  cursor visible (ESC e) and blinking (Cursconf(3)); a key exits.
 *
 * Spec: docs/api/bios/_vt52.md (sections 3.3, 6, 7.1, 8, 13 Q6),
 * docs/api/xbios/15-cursconf.md, docs/api/vdi/_escape.md. TASK/CON04.md.
 * Build: sdk/bin/newtos-cc -nap -o VT52PI.NAP sdk/examples/vt52pi.c
 */
#include <gem.h>
#include <osbind.h>

#define ESC 0x1b
#define MAXCOLS 1024

static short vh;          /* VDI virtual workstation handle */
static short ncols, nrows;

static void out(int c) { Bconout(2, c & 0xff); }

static void s2(const char *s)
{
    while (*s)
        out((unsigned char)*s++);
}

static void esc(int c)
{
    out(ESC);
    out(c);
}

/* ESC Y: row, then column, each as one byte = 32 + value. */
static void escy(int row, int col)
{
    out(ESC);
    out('Y');
    out(32 + row);
    out(32 + col);
}

static void num(int v)
{
    char b[12];
    int n = 0;
    if (v < 0) {
        out('-');
        v = -v;
    }
    do {
        b[n++] = (char)('0' + v % 10);
        v /= 10;
    } while (v);
    while (n)
        out(b[--n]);
}

static int min(int a, int b) { return a < b ? a : b; }

static void hex2(int v)
{
    static const char h[] = "0123456789ABCDEF";
    out(h[(v >> 4) & 15]);
    out(h[v & 15]);
}

/* Write one whole row from column 0; `buf` holds ncols cells. With wrap
 * off the last cell is written once and the cursor stays there. */
static void row_out(int row, const char *buf)
{
    int i;
    escy(row, 0);
    for (i = 0; i < ncols; i++)
        out((unsigned char)buf[i]);
}

static void fill(char *buf, int c)
{
    int i;
    for (i = 0; i < ncols; i++)
        buf[i] = (char)c;
}

/* Ruler: digits of every multiple of 10 starting at that column. */
static void ruler_numbers(int row)
{
    static char buf[MAXCOLS];
    int c, n, v;
    char d[8];
    fill(buf, ' ');
    for (c = 0; c < ncols; c += 10) {
        v = c;
        n = 0;
        do {
            d[n++] = (char)('0' + v % 10);
            v /= 10;
        } while (v);
        for (v = 0; v < n && c + v < ncols; v++)
            buf[c + v] = d[n - 1 - v];
    }
    row_out(row, buf);
}

/* Ruler marks: '|' every 10, '+' every 5, '.' otherwise; '#' at the
 * guide columns g1/g2 (-1 = none). */
static void ruler_marks(int row, int g1, int g2)
{
    static char buf[MAXCOLS];
    int c;
    for (c = 0; c < ncols; c++)
        buf[c] = (char)(c % 10 == 0 ? '|' : c % 5 == 0 ? '+' : '.');
    if (g1 >= 0 && g1 < ncols)
        buf[g1] = '#';
    if (g2 >= 0 && g2 < ncols)
        buf[g2] = '#';
    row_out(row, buf);
}

static void text(int row, const char *s)
{
    escy(row, 0);
    s2(s);
}

int main(void)
{
    short work_in[11] = { 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2 };
    short work_out[57];
    short wc, hc, wb, hb;
    short r4, c4a, c4b, r5, c5, r5b, c5b;
    static char buf[MAXCOLS];
    int i, t3, t4a, t4b, last;

    if (appl_init() < 0)
        return 1;
    vh = graf_handle(&wc, &hc, &wb, &hb);
    v_opnvwk(work_in, &vh, work_out);
    if (vh <= 0) {
        appl_exit();
        return 1;
    }

    wind_update(BEG_UPDATE);
    graf_mouse(M_OFF, 0L);
    v_enter_cur(vh);               /* ESC E ESC e CR: alpha screen, home */
    esc('f');                      /* cursor off while drawing */

    vq_chcells(vh, &nrows, &ncols);
    if (ncols > MAXCOLS)
        ncols = MAXCOLS;
    last = ncols - 1;
    t3 = min(223, last);           /* ESC Y: byte 0xFF is the highest */
    t4a = min(230, last);
    t4b = last;

    /* r0: title with the geometry vq_chcells reported. */
    text(0, "VT52PI CON04 cols=");
    num(ncols);
    s2(" rows=");
    num(nrows);
    if (ncols < 231)
        s2(" (<231 cols: T3/T4 clamp to last col)");

    ruler_numbers(1);
    ruler_marks(2, -1, -1);

    /* T1: wrap off, the state the program found (no ESC w sent). */
    text(3, "T1 wrap off: next row ENDS 'abcdZ' at last col, row below all '-'");
    fill(buf, '-');
    row_out(4, buf);
    row_out(5, buf);
    escy(4, 0);
    for (i = 0; i < ncols - 5; i++)
        out('.');
    s2("abcdeWXYZ");               /* W X Y Z overwrite the last cell */

    /* T2: wrap on. */
    text(6, "T2 wrap on (ESC v): next row ends 'abcde', row below 'WXYZ---'");
    row_out(7, buf);
    row_out(8, buf);
    esc('v');
    escy(7, 0);
    for (i = 0; i < ncols - 5; i++)
        out('.');
    s2("abcdeWXYZ");               /* 'e' on the last cell, then wrap */
    esc('w');

    /* T3: ESC Y to column 223. */
    text(9, "T3 ESC Y col ");
    num(t3);
    s2(" (byte 0x");
    hex2(32 + t3);
    s2("): 'Y' right under the '#' above it");
    ruler_marks(10, t3, -1);
    fill(buf, '.');
    row_out(11, buf);
    escy(11, t3);
    out('Y');

    /* T4: vs_curaddress (1-based row, column) beyond 223. */
    text(12, "T4 vs_curaddress col ");
    num(t4a);
    s2(" and ");
    num(t4b);
    s2(": 'V', 'L' right under the '#'s");
    ruler_marks(13, t4a, t4b);
    row_out(14, buf);
    vs_curaddress(vh, 15, t4a + 1);
    vq_curaddress(vh, &r4, &c4a);
    out('V');
    vs_curaddress(vh, 15, t4b + 1);
    vq_curaddress(vh, &r4, &c4b);
    out('L');

    /* T5: vs_curaddress beyond the screen clamps. */
    text(15, "T5 clamp (999): 'K' at end of next row, 'C' bottom-right corner");
    row_out(16, buf);
    vs_curaddress(vh, 17, 999);
    vq_curaddress(vh, &r5, &c5);
    out('K');
    vs_curaddress(vh, 999, 999);
    vq_curaddress(vh, &r5b, &c5b);
    out('C');

    /* Read-back line (vq_curaddress, 1-based). */
    text(17, "vq_curaddress: T4 V=");
    num(r4);
    out(',');
    num(c4a);
    s2(" L=");
    num(r4);
    out(',');
    num(c4b);
    s2(" T5 K=");
    num(r5);
    out(',');
    num(c5);
    s2(" C=");
    num(r5b);
    out(',');
    num(c5b);
    escy(18, 0);
    s2("must be: V=15,");
    num(t4a + 1);
    s2(" L=15,");
    num(ncols);
    s2(" K=17,");
    num(ncols);
    s2(" C=");
    num(nrows);
    out(',');
    num(ncols);

    /* T6: visible, blinking cursor; wait for a key. */
    text(20, "T6 cursor must BLINK right after the arrow. Any key exits.");
    escy(21, 0);
    s2("--> ");
    Cursconf(3, 0);                /* M_CFLASH on: blinking */
    esc('e');                      /* show (disab_cnt = 0) */

    (void)evnt_keybd();

    v_exit_cur(vh);                /* ESC H ESC f ESC E */
    graf_mouse(M_ON, 0L);
    wind_update(END_UPDATE);
    form_dial(FMD_FINISH, 0, 0, 0, 0, 0, 0, work_out[0] + 1, work_out[1] + 1);
    v_clsvwk(vh);
    appl_exit();
    return 0;
}
