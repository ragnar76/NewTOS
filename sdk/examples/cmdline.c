/*
 * cmdline.c -- print the command line the launcher handed this program.
 *
 * A native NewTOS console program (`.NCP`, ADR-018). It reads its own
 * BasePage -- found the classic way, through the OSHEADER's `os_run` cell
 * (`_sysbase` $4F2 -> OSHEADER +0x28 -> the current-PD cell; since ARCH03
 * stage 1 the basepage is the start of the program's TPA block, no longer
 * VA 0) -- and prints the command tail out of `p_cmdlin` (offset 0x80,
 * docs/api/gemdos/_basepage.md / tos-types/src/basepage.rs): a length byte
 * followed by that many characters.
 *
 * It is the acceptance program for DESK19 (dropping a selection on a program
 * icon is a launch with arguments, not a copy, EmuTOS deskfun.c:958-970): drop
 * a file on it and the line it prints is what `build_selected_path`
 * (deskfun.c:903-928) built -- a bare file name or a full path, depending on
 * the desktop configuration's *Parameter* setting.
 *
 * It also prints the current drive + directory, so the same run shows which
 * directory `do_aopen` (desksupp.c:884-905) chose -- the *Default dir* switch
 * DESK18 could not make observable on its own.
 *
 * Calls used, all against docs/api/:
 *   Dgetdrv  GEMDOS 0x19  docs/api/gemdos/drive/19-dgetdrv.md
 *   Dgetpath GEMDOS 0x47  docs/api/gemdos/directory/47-dgetpath.md
 *   Cconws   GEMDOS 0x09  docs/api/gemdos/console/09-cconws.md
 */
#include <newtos.h>

static NLONG Dgetdrv(void)              { return gemdos0(0x19); }
static NLONG Dgetpath(char *b, NLONG d) { return gemdos2(0x47, (NLONG)b, d); }

/* Offset of `p_cmdlin` in the BasePage (tos-types/src/basepage.rs). */
#define P_CMDLIN 0x80

/* Our own BasePage: `*(_sysbase->os_run)` (OSHEADER +0x28, the pointer to
 * the current-PD cell; docs/api/structures/osheader.md). */
static const unsigned char *own_basepage(void)
{
    unsigned long sysbase = *(volatile unsigned int *)0x4F2UL;
    unsigned long run_cell = *(volatile unsigned int *)(sysbase + 0x28);
    return (const unsigned char *)(unsigned long)*(volatile unsigned int *)run_cell;
}

/* Append a NUL-terminated string to `dst`, returning the new end index. */
static int puts_at(char *dst, int j, const char *s)
{
    while (*s)
        dst[j++] = *s++;
    dst[j] = '\0';
    return j;
}

int main(void)
{
    /* Our BasePage; the command tail starts at 0x80. */
    const unsigned char *bp = own_basepage();
    const unsigned char *tail = bp + P_CMDLIN;
    unsigned int len = tail[0];

    char line[256];
    char path[128];
    NLONG drv;
    unsigned int i;
    int j;

    /* "current drive + directory" -- which directory the launcher chose. */
    drv = Dgetdrv();
    path[0] = '\0';
    Dgetpath(path, 0);

    j = puts_at(line, 0, "CMDLINE: cwd ");
    line[j++] = (char)('A' + (int)drv);
    line[j++] = ':';
    line[j] = '\0';
    j = puts_at(line, j, path[0] ? path : "\\");
    j = puts_at(line, j, "\r\n");
    Cconws(line);

    /* The command tail itself: length byte, then that many characters. */
    j = puts_at(line, 0, "CMDLINE: len ");
    if (len >= 100)
        line[j++] = (char)('0' + (len / 100) % 10);
    if (len >= 10)
        line[j++] = (char)('0' + (len / 10) % 10);
    line[j++] = (char)('0' + len % 10);
    line[j] = '\0';
    j = puts_at(line, j, " arg \"");
    for (i = 0; i < len && j < 240; i++)
        line[j++] = (char)tail[1 + i];
    line[j] = '\0';
    j = puts_at(line, j, "\"\r\n");
    Cconws(line);

    return 0;
}
