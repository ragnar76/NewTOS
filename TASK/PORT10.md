# PORT10 — libc: `system()`/`spawn*` linken nicht, `printf("%f")` stuerzt ab

Status: **OFFEN** · Prioritaet: **P1** (blockiert den auslieferbaren Doom-Bau) · Typ: SDK / libc — aus [PORT06](PORT06.md)

Abhaengigkeiten: —

## Sachverhalt

Aus PORT06 Schritt 1 (`TASK/PORT06-STATUS-UNFINISHED.MD`,
`measurements/measurement-2026-10-02-port06/`):

1. **`system()` linkt nicht.** mintlibs `system()` zieht `spawnvp` nach;
   `spawnve.c`/`spawnvp.c` kompilieren unter LP64 nicht (`va_list`-Arithmetik,
   `sdk/libc/expected-failures.txt:26-27`, `docs/sdk/07-mintlib-limits.md:22`).
   Dazu fehlen `vfork`, `__waittime`, `__waitval`. doomgeneric ruft
   `system()` nur in `ZenityErrorBox` — der Bau bricht trotzdem beim Linken ab.
2. **`printf("%f")` faellt in `__printf_fp`** — Data Abort aus EL0, FAR
   wechselnd. Trifft Dooms timedemo-Schlusszeile und jedes Programm, das
   Gleitkomma ausgibt.
3. **Nebenbei:** `intptr_t`/`uintptr_t` sind unter LP64 32 Bit
   (`mintlib/include/stdint.h`, Weiche ueber `__MSHORT__`).

## Arbeit

1. `spawn*`/`system()` so umsetzen, wie mintlib es auf klassischem TOS ohne
   MiNT tut (synchroner `Pexec`), LP64-sauber; nur die arch-Grenze anpassen,
   libc nicht neu schreiben.
2. `__printf_fp`-Absturz mit Ursache belegen und beheben.
3. `intptr_t`/`uintptr_t` auf 64 Bit, Folgen pruefen.

Abnahme: `sdk/apps/build-doom.sh` linkt ohne Diagnose-Ersatz; Hosttests der
libc gruen; ein Testprogramm ruft `system()` und `printf("%f")` auf NewTOS
erfolgreich.
