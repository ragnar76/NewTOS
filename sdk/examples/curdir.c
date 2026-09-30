/*
 * curdir.c -- prove that a launched program inherits a working directory.
 *
 * A native NewTOS console program (`.NCP`, ADR-018). It reports the current
 * drive and directory, then opens a file by a **bare relative name** and
 * prints what it got. Which copy of that file it finds is the whole point:
 * the name carries no path, so the answer is decided entirely by the current
 * directory the launcher set before dropping to EL0.
 *
 * Staged by tools/make-sd-image.sh as C:\SUB\CURDIR.NCP, with two different
 * files of the same bare name:
 *
 *     C:\PROOF.TXT      -> "PROOF: C:\ (root)"
 *     C:\SUB\PROOF.TXT  -> "PROOF: C:\SUB"
 *
 * so the printed line names the directory the program actually ran in.
 *
 * Reached only by a double-click (make-sd-image.sh keeps it out of \AUTO\ on
 * purpose -- the AUTO path has its own directory and this program is there to
 * tell the two apart), so both of its exits hold the report with
 * newtos_pause() (TASK/T13).
 *
 * Calls used, all against docs/api/:
 *   Dgetdrv  GEMDOS 0x19  docs/api/gemdos/drive/19-dgetdrv.md
 *   Dgetpath GEMDOS 0x47  docs/api/gemdos/directory/47-dgetpath.md
 *   Fopen    GEMDOS 0x3D  docs/api/gemdos/file/3d-fopen.md
 *   Fread    GEMDOS 0x3F  docs/api/gemdos/file/3f-fread.md
 *   Fclose   GEMDOS 0x3E  docs/api/gemdos/file/3e-fclose.md
 *   Cconws   GEMDOS 0x09  docs/api/gemdos/console/09-cconws.md
 */
#include <newtos.h>

/*
 * Three-argument GEMDOS trap. newtos.h ships gemdos0/1/2 only (Stage 1 needed
 * no more), and Fread takes three; kept local to this example rather than
 * widening the SDK header as a side effect.
 */
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

static NLONG Dgetdrv(void)               { return gemdos0(0x19); }
static NLONG Dgetpath(char *b, NLONG d)  { return gemdos2(0x47, (NLONG)b, d); }
static NLONG Fopen(const char *p, NLONG m) { return gemdos2(0x3D, (NLONG)p, m); }
static NLONG Fclose(NLONG h)             { return gemdos1(0x3E, h); }
static NLONG Fread(NLONG h, NLONG n, char *b) { return gemdos3(0x3F, h, n, (NLONG)b); }

/* Append a signed decimal to a NUL-terminated buffer. */
static void putnum(char *dst, NLONG v)
{
    char tmp[24];
    int i = 0, j = 0;
    while (dst[j])
        j++;
    if (v < 0) {
        dst[j++] = '-';
        v = -v;
    }
    do {
        tmp[i++] = (char)('0' + (v % 10));
        v /= 10;
    } while (v);
    while (i)
        dst[j++] = tmp[--i];
    dst[j] = '\0';
}

int main(void)
{
    char line[160];
    char path[128];
    char data[64];
    NLONG drv, h, n;
    int i, j;

    /* "current drive + directory" -- the state the launcher was to set. */
    drv = Dgetdrv();
    path[0] = '\0';
    Dgetpath(path, 0);

    line[0] = '\0';
    j = 0;
    { const char *s = "CURDIR: drive "; while (*s) line[j++] = *s++; }
    line[j++] = (char)('A' + (int)drv);
    line[j++] = ':';
    for (i = 0; path[i] && j < 120; i++)
        line[j++] = path[i];
    line[j++] = '\r';
    line[j++] = '\n';
    line[j] = '\0';
    Cconws(line);

    /* The bare relative name: no drive, no path, no leading separator. */
    h = Fopen("PROOF.TXT", 0);
    if (h < 0) {
        line[0] = '\0';
        { char *d = line; const char *s = "CURDIR: Fopen(\"PROOF.TXT\") = "; j = 0;
          while (*s) d[j++] = *s++; d[j] = '\0'; }
        putnum(line, h);
        { j = 0; while (line[j]) j++; line[j++] = '\r'; line[j++] = '\n'; line[j] = '\0'; }
        Cconws(line);
        newtos_pause();
        return 1;
    }

    n = Fread(h, (NLONG)sizeof(data) - 1, data);
    Fclose(h);
    if (n < 0)
        n = 0;
    data[n] = '\0';
    /* Trim the file's own CR/LF so the report stays one line. */
    for (i = 0; data[i]; i++)
        if (data[i] == '\r' || data[i] == '\n') {
            data[i] = '\0';
            break;
        }

    line[0] = '\0';
    j = 0;
    { const char *s = "CURDIR: read \""; while (*s) line[j++] = *s++; }
    for (i = 0; data[i] && j < 140; i++)
        line[j++] = data[i];
    line[j++] = '"';
    line[j++] = '\r';
    line[j++] = '\n';
    line[j] = '\0';
    Cconws(line);

    newtos_pause();
    return 0;
}
