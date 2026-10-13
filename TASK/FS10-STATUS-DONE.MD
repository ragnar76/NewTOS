# FS10 — Quittung: `Fseek` auf Zeichengeraete-/Standard-Handles liefert 0

Datum 2026-10-13 · Basis `main`, HEAD `6b8468bf`, Baum vorher sauber
· nicht committet · Belege
[measurements/measurement-2026-10-13-fs10/](../measurements/measurement-2026-10-13-fs10/RUNINFO.TXT)
· Ticket [FS10](FS10.md) · Spec `docs/api/gemdos/file/42-fseek.md`
§„Character-device and standard handles (CORRECTION 2026-10-13)“ (freigegeben, `6b8468bf`)

## Umsetzung (`tos-api/src/bdos/fs/file.rs`)

* `resolve(handle: i16)`: Handle als vorzeichenbehaftetes WORD, Aufloesung wie
  EmuTOS `osif` (`bdos/bdosmain.c:571-602`): `>= 6` → `sft`-Eintrag, `0..5` →
  `p_uft`, `< 0` → das Geraet selbst; ein BIOS-Geraet unter `-3` ist ungueltig
  (`:601-602`); ein Handle ausserhalb der Tabelle ebenfalls (Vertragszeile
  `>= NUMHANDLES`).
* `fseek`: Geraet → `0` unabhaengig von Offset und Modus (auch ungueltiger
  Modus, `xlseek` laeuft nicht, `:642`); ungueltig → `EIHNDL`; Datei → wie bisher.
* `fread`/`fwrite`/`fdatime`: Handle per `arg_i16` statt `arg_u16`.

Keine fruehere Verengung: `TrapArgs` traegt die Register als volle `u64`
(`tos-api/src/trap.rs`), der GEMDOS-Dispatcher reicht unveraendert durch;
`arg_i16` nimmt die unteren 16 Bit, ein vorzeichen- **und** ein nullerweitertes
WORD ergeben dasselbe (Hosttests pruefen beide Kodierungen).
`docs/ARCHITECTURE.md` §ABI Rules legt nur „Argumente in x0–x7 in TOS-Reihenfolge“
fest, keine Erweiterungsregel fuer WORD-Argumente.

## Verhalten von `-2`/`-3` bei `Fread`/`Fwrite` nach der Aenderung

Vorher: `65534`/`65533` → `EIHNDL`. Jetzt: wie `-1` auf die Konsole
(`console_read`/`console_write`). EmuTOS schreibt dort per `Bconout` auf
AUX/PRN bzw. liest per `conin`/`cgets` vom jeweiligen Geraet
(`bdosmain.c:608-640`) — diese Unterscheidung ist bewusst NICHT Teil von FS10
(eigenes Ticket des Leads). Ebenfalls neu, aus derselben Aufloesungsregel:
`Fforce(h, -4)` gefolgt von `Fread/Fwrite/Fseek(h)` → `EIHNDL` (vorher Konsole),
wie EmuTOS `:601-602`.

## Tests

`tos-api/tests/fs10_fseek_device_handles.rs`, 9 Tests (T1–T13 der Spec,
`Fforce` auf `-3`/`-4`, `Fread`/`Fwrite` mit negativen Handles):
vor der Aenderung 2 gruen / 7 rot (jeder Geraetefall `-37`), danach 9/0.

## Basislinien (gehalten)

Hosttests 1277/0/1 (= 1268 + 9 neue) · Release beider Boards tos-api 64 /
kernel 94, 0 Fehler (frisch nach `cargo clean … --release`) · Clippy 76/125
beide Feature-Saetze · Driftprobe 31 · SDK-Hosttests 19/0/0.

## QEMU raspi4b (on / off / strict)

FSEEKCON **PASS** in allen drei Modi. Stichprobe: APITEST 106/0/5, Doom
VERIFIED, ATEXIT PASS, KBDVEC+EmuCON PASS, S2ALL wie a13 (nur der bekannte
veraltete MEM03/ENVKID-Fehler); Desktop/TeraDesk/QED/EmuCON-Dumps bytegleich
zu a13. Doom-Dumps t30/t45 weichen ab — Zeitjitter, belegt durch zwei
Wiederholungen mit dem a13-Kernel selbst (RUNINFO).

## Offen / nicht Teil

* `docs/STATUS.md` (Basislinie Hosttests 1277, Eintrag *Recent Decisions*)
  beim Commit nachziehen.
* AUX/PRN im `Fread`/`Fwrite`-Weg (eigenes Ticket).
* crt0 auf mintlibs `_main` (Folgeschritt laut FS10).
* Nicht gefahren: echte Pi 4, virt-QEMU-Laeufe, `asid=on`.
