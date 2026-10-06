# PORT-WAITH — mintlib `<sys/wait.h>` auf AArch64 — ERLEDIGT

Nichts committet. Beleg: `measurements/measurement-2026-10-06-waith/RUNINFO.TXT`.

## Befund (Quelle, nicht geraten)

- `include/sys/wait.h` liest das Statuswort über `struct __wait`
  (`junk:16, retcode:8, coredump:1, termsig:7`) — big-endian-Reihenfolge.
  Auf AArch64 (AAPCS64: erstes Feld = niederwertigste Bits) liegt `retcode`
  in Bit 16..23: `WEXITSTATUS` immer 0, `WIFEXITED` immer wahr,
  `WIFSIGNALED`/`WIFSTOPPED` nie.
- Das Statuswort selbst baut `unix/wait4.c` rein arithmetisch (Bytetausch:
  Code in Bit 8..15, Core Bit 7, Signal Bit 0..6; dieselbe Lage wie
  `W_EXITCODE`/`W_STOPCODE`). `wait.c`/`wait3.c`/`waitpid.c` rufen nur
  `wait4`. Bitfeldnutzer in mintlib: `sunrpc/`, `unix/test-vfork.c`,
  `test-skeleton.c` — werden nicht gebaut. Bibliothekscode braucht keine
  Änderung (neu gebautes `libc.a` bytegleich).

## Änderung

- `sdk/libc/overlay/include/sys/wait.h` (neu): unveränderter Header, nur
  `struct __wait` bei `__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__` in
  umgekehrter Feldreihenfolge — gleiche Bitlage, Makros unverändert.
- `sdk/libc/tests/waith-host.sh` + `waith-probe.c`, `waith-host.c`,
  `waith-host-mintbind.h` (neu): 74 Prüfungen (Makros auf bekannten Worten,
  `union wait` beide Richtungen, mintlibs unverändertes `wait4.c`/`waitpid.c`/
  `wait.c` mit dem `__waitval` aus `tfork`); rot mit Original-Header 28/74.
- `sdk/run-host-tests.sh`: `waith-host` eingehängt (13 → 14).
- `sdk/libc/build-libc.sh`: Overlay-Liste im Kopfkommentar.
- Doku: `docs/sdk/07-mintlib-limits.md` neuer §9, §8 berichtigt (VFORKT hatte
  das Statuswort geprüft, nicht `WEXITSTATUS`; TOS-Zweig von `system()` gibt
  den rohen Exitcode zurück, kein Wait-Status), §7-Zeile, Kopf;
  `docs/sdk/05-abi-pitfalls.md` B11 Verweis.

## Prüfungen

| | Basis (arch03-4) | jetzt |
|---|---|---|
| Hosttests | 1165/0/1 | 1165/0/1 |
| Release-Warnungen tos-api/kernel | 64/94 beide | 64/94 beide |
| clippy tos-api/kernel rpi4, virt | 76/125 | 76/125 |
| Drift | 31 | 31 |
| SDK-Hosttests | 13/0/0 | 14/0/0 |
| QEMU raspi4b (Schutz an) VFORKT allein | PASS (Statuswort) | PASS, `WEXITSTATUS == 42`, `== 7`, `system()` `WIFEXITED && WEXITSTATUS == 127` |
| QEMU raspi4b S2ALL | PASS | PASS |

## Binärdateien

- `libc.a` neu gebaut, bytegleich. Apps (QED, TeraDesk, Doom, APITEST) nicht
  neu gebaut — keine nutzt `W*`-Makros oder `union wait`; nichts muss auf die
  Karte.
- Sonden: nur `VFORKT.NAP` geändert (neue Quelle im Messordner); die übrigen
  24 neu gebauten Sonden bytegleich zu arch03-4.

## Offen

- `system()` im TOS-Zweig liefert den rohen Exitcode (mintlib
  `system_hack`), `WEXITSTATUS(system(...))` ist dort falsch — mintlib-
  Verhalten, unverändert, in 07 §8 dokumentiert.
- `docs/STATUS.md` nicht nachgetragen (Lead beim Commit).
