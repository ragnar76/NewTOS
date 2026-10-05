# ARCH03 Stufe 2 — Quittung

**Status: IMPLEMENTIERT (a–e) · in QEMU auf beiden Boards belegt · Abnahme
durch den Tester offen · Pi offen · f (PROC02) nicht gemacht** · 2026-10-05 · Zweig `arch03`, Basis HEAD `f293b66`, Baum
vorher sauber, nichts committet · Belege
[measurements/measurement-2026-10-05-arch03-2/](../measurements/measurement-2026-10-05-arch03-2/)
(`checks/` statische Prüfungen je Teilschritt, `serial/` + `screendumps/`
QEMU-Läufe, `method/` Skripte, `bin/` Kerne und Programme).

| Teilschritt | Stand |
|---|---|
| a) E8b PRGFLAGS-Notiz | **fertig, belegt** |
| b) Pexec 5/6/7 | **fertig, belegt** (Prüfpunkt gemeinsam mit c/d, s. u.) |
| c) Ptermres | **fertig, belegt** |
| d) Mehrfachbesitz + E15 | **fertig, belegt** (eine Sondenerwartung nachgezogen, Nachweis in e) |
| e) mintlib vfork/thread.c, system()-Overlay weg | **fertig, belegt** (PROC01 erledigt) |
| f) PROC02 | **nicht gemacht** (s. „Offen“) |

## Teilschritt a — PRGFLAGS als ELF-Notiz (E8b)

Spec zuerst: `docs/api/gemdos/process/4b-pexec.md` (NewTOS-Notizen,
„`PRGFLAGS` of a NewTOS program“). Notiz `.note.newtos.prgflags`, Name
`"NewTOS"`, Typ 1, ein `u32`; Kopfworte 4 Byte, Name/Desc auf die
`PT_NOTE`-Ausrichtung aufgefüllt (4, bei `p_align` 8 auf 8). **Entscheidung,
offen gelegt:** die gepinnte gABI-Fassung nennt ELF64-Notizen „8-byte words“;
`rust-lld`/`llvm-readobj` schreiben und lesen 4-Byte-Felder — die Notiz folgt
dem Werkzeug (geprüft: `llvm-readobj --notes` dekodiert sie richtig).

| Datei | was |
|---|---|
| `tos-api/src/bdos/process/elf.rs` | `PT_NOTE` wird erfasst (≤ 4 Köpfe, je ≤ 4 KiB, in der Datei), `find_prgflags_note`, `prgflags_of`; Fehler `BadNoteSegment`/`BadNote` → `EPLFMT` |
| `tos-api/src/bdos/process/loader.rs` | Schritt 1c: Notiz vor jeder Zuteilung lesen (`read_prgflags`), sonst `PF_STANDARD`; `p_flags` aus der Notiz; Pool von TPA und Umgebung nach `PF_TTRAMLOAD` (`pool_mode`, EmuTOS `alloc_env`/`alloc_tpa`); `ProcessSetup` bündelt die Ladeparameter |
| `tos-api/src/bdos/process/arena.rs` | `alloc_tpa(prgflags)` |
| `sdk/crt0.S`, `sdk/libc/crt0.S` | Notiz, Standard `PF_STANDARD`, `-DNEWTOS_PRGFLAGS=` |
| `sdk/newtos.ld` | `.note.newtos` im `text`-Segment + `PT_NOTE`-Kopf |
| `sdk/build.sh` | Umgebungsvariable `PRGFLAGS` |
| `tools/xtask/src/main.rs` | `cargo xtask prgflags <datei> [<wert>]` (lesen / an Ort und Stelle setzen; benutzt `elf.rs` per Pfad) |
| Doku | `docs/COMPATIBILITY.md` (neuer Eintrag), `docs/sdk/03-first-programs.md` |
| Tests | neu `tos-api/tests/loader_prgflags.rs` (10), xtask `prgflags_tests` (3) |

Prüfungen `checks/s2a-*`: Hosttests **1063/0/1** (1050 + 13), Release frisch
`tos-api` **64**, `kernel` **94** beide Boards, Clippy **80/125** beide
(ein `too_many_arguments` durch `ProcessSetup` behoben), Driftprobe **31**,
SDK **13/0/0**.

QEMU (`checks/s2a-runs.txt`, `serial/s2a-*`, `screendumps/s2a-*`):
Desktop, TeraDesk, QED, APITEST-Bild **pixelgleich** mit Stufe 1; Doom
`VERIFIED via EdDI+scale+vro_cpyfm`, Demo läuft; APITEST **106/0/5**;
S2ALL: **PFLAGS: PASS** ×2 (PFLAGS1 mit `PRGFLAGS=0x10000004` → `p_flags`
gelesen, TPA und Umgebung im ST-RAM, `Malloc` im TT-RAM; PFLAGS2 mit
`cargo xtask prgflags … 0x7` umgestellt → TT-RAM), P34 PASS, MEM03 PASS;
PORT10 42/44 (dieselben 2 Umleitungsfehler wie seit PORT10d, PROC02);
CRASHTST PASS (raspi4b, virt); KBDVEC PASS + EmuCON (`run oldexec.ncp` →
Fehler 66); virt-Selbsttests 9 PASS + `sleepwake_switch` mit denselben
Prüfungen FAIL wie Stufe 1 (vorbestehend); virt Desktop.

Alle Programme des Baums tragen jetzt die Notiz (QED, TeraDesk, Doom,
APITEST neu gebaut, `0x00000007`); `HELLO.NCP` (handgebaut) hat keine und
lädt mit `PF_STANDARD`.

## Teilschritte b, c, d — Prüfpunkte gemeinsam (offen gelegt)

c lag schon im Baum, als b das erste Mal gefahren wurde; b und c sind deshalb
zusammen geprüft (`s2bc`), danach b+c+d (`s2bcd`). Ein getrennter b-Lauf
existiert nicht.

**Spec zuerst:** `docs/api/gemdos/process/4b-pexec.md` (NewTOS-Notizen 2b und
2d), `31-ptermres.md` (2c), `docs/api/gemdos/memory/49-mfree.md` und
`4a-mshrink.md` (2d, „several owners, and only an owner frees“),
`4c-pterm.md` (2d), `docs/COMPATIBILITY.md` (neuer Eintrag E15/TSR).

| Datei | was |
|---|---|
| `tos-api/src/bdos/process/loader.rs` | `install_slot` (Seite 0, Wurzel, Slot-Metadaten, ausfaktorisiert), `create_basepage` (Pexec 5/7: Umgebung, TPA nach Flags, `init_pd_fields`, nur die 256 Basepage-Bytes) |
| `tos-api/src/bdos/process/pexec.rs` | Modi 5/6/7 im Dispatch (`path` als Flags für 7, nie als String), `create`, `go(handle, then_free)`: nur ein Eigentümer startet (sonst `EIMBA`), Kind wird Mitbesitzer von Basepage+Umgebung, Modus 6 übergibt (`hand_over`), Pexec-4-Kind teilt den Speicherraum (`space_pd`); `start_of` (Einsprung ELF-Entry bzw. `p_tbase`, Stapel immer bei `p_hitpa` wie EmuTOS `proc_go`); `release_unstarted_at`; `UnsupportedMode` entfernt, `NoOwnerRoom` |
| `tos-api/src/bdos/process/pterm.rs` | `terminate_resident(blkln, rc)`: `xsetblk(0, run, blkln)` (= `traps::mshrink`, Ergebnis ignoriert, D-006), unstarted Kinder auch bei Ptermres frei |
| `tos-api/src/bdos/process.rs` | `Disposition::Terminate { reserve: Option<blkln> }` (Kodierung rc 16 Bit, blkln Bit 16..48), `space_pd`, `current_space` |
| `tos-api/src/bdos/mem/region.rs` (neu) | weitere Eigentümer je Block (FreeMiNT-Links über `m_own`): `attach`, `detach` (Freed/Kept, Etikett wandert), `hand_over`, `forget`, `detach_all` (Pterm), `keep_resident` (Ptermres → TSR-Halter), `is_owner`, `links` |
| `tos-api/src/bdos/mem/{traps,free_all_owned,mod}.rs` | TSR-Halter `tsr_owner`/`is_resident`; `forget` bei jeder Freigabe und frischer Zuteilung; Sweeps über `region` |
| `tos-api/src/gemdos/memory.rs` | E15: `Mfree`/`Mshrink` nur für einen Eigentümer (eigener PD oder geteilter Speicherraum), `Mfree` löst nur den eigenen Link, freigegeben mit dem letzten; AES-Meldung nur bei echter Freigabe |
| `tos-api/src/aes/resource.rs` | `on_mfree`-Kommentar: Stufe-1-Begründung durch E15/Links ersetzt (Markieren aller Einträge des Schlüssels bleibt, jetzt nur bei echter Freigabe) |
| `kernel/src/arch/aarch64/exceptions.rs` | `handle_pexec`: `path` nur für 0/3 lesen, Kommando/Umgebung auch für 5/7, Fehler über `PexecError::errno`, **Kind-Slot `Running`** (s. Befund); `handle_pterm`/`Psigreturn`: `blkln` |
| `kernel/src/selftest_d7.rs` | Ptermres-Selbsttest auf die neue Semantik (Block bleibt zugeteilt, Eigentümer TSR, TPA auf `blkln` geschrumpft) |
| Tests | neu `tos-api/tests/arch03_pexec_567_ptermres.rs` (7), `arch03_region_owners.rs` (11) |

**Befund unterwegs (Stufe-1-Latenz, in b behoben):** Der `Pexec`-Startweg
setzte den Kind-Slot nie auf `Running`; ein laufendes Kind galt für
`PE_GO` als „geladen, nicht gestartet“, und `Pexec(4, eigener PD)` startete
P567 ein zweites Mal (`serial/s2bc-rpi4-s2all-serial.log`, P567 FAIL,
S2ALL bis zur Zeitgrenze). Jetzt markiert `handle_pexec` wie Kernelstarter
und Scheduler. Eine Korrektur, im nächsten Prüfpunkt PASS.

**Sondenerwartung nachgezogen (d):** OWNPAR erwartete, dass ein Pexec-4-Kind
nach `Mfree(env)` kein Eigentümer mehr ist. Laut Spec teilt es den
Speicherraum des Aufrufers (E6, FreeMiNT `FORK_SHAREVM`): das zweite `Mfree`
löst den Link des Aufrufers und gibt frei — so wie FreeMiNT beide Anhänge
löst. Das Produkt verhält sich spezifikationsgemäß, die Sonde ist korrigiert
(`method/tests/ownkid.c`, `ownpar.c`); Nachweis im Prüfpunkt von e.

Prüfungen:

| | s2bc | s2bcd |
|---|---|---|
| Hosttests | 1070/0/1 | **1081/0/1** |
| Release `tos-api`/`kernel` beide Boards | 64/94 | 64/94 |
| Clippy beide | 80/125 | 80/125 |
| Driftprobe | 31 | 31 |
| SDK-Hosttests | 13/0/0 | 12/1/0 — `system-host` prüfte das in e entfernte Overlay; in e angepasst, 13/0/0 |
| QEMU | Desktop/TeraDesk/QED/APITEST-Bild pixelgleich Stufe 1, Doom VERIFIED, APITEST 106/0/5, CRASHTST PASS ×2, KBDVEC PASS + EmuCON, Selbsttests 9 PASS (D7 neu) + sw3b wie vorher, PORT10 42/44; **S2ALL: P567 FAIL** (Befund oben) | dasselbe; **S2ALL: PFLAGS ×2, P34, MEM03, P567, TSR PASS; OWN FAIL** (Sondenerwartung, oben) |

Belege TSR (s2bcd): `Ptermres(0x1a70, 0x31)`; residenter Code dreimal
gerufen (1022, 2022, dann aus TSRUSE 3022) — Daten, BSS, Rodata und der
`Malloc`-Block bleiben; größter TT-Block vorher `0x3a568ff0`, nachher
`0x3a566590` (nur `blkln` + Umgebung bleiben); `Mfree` von TSR-PD und
TSR-Block = `EIMBA`. Pexec 5/6/7 (P567): Basepage-Felder, Pool nach Flags,
Start bei `p_tbase` mit `x0` = PD, `*(os_run)` = Kind, `p_parent`, Modus 6
gibt Speicher an das Kind (danach `EIMBA`, ST-RAM wie vorher), Modus 7 mit
`0x10000007` (Pfad nie als String gelesen), `Pexec 8` = `EINVFN`.

## Teilschritt e — mintlib `vfork`, `thread.c`, `system()` unverändert (PROC01)

| Datei | was |
|---|---|
| `sdk/libc/newtos-arch/vfork.S` (neu) | AArch64-Port von mintlibs `unix/vfork.S`: MiNT-Zweig (`Pvfork`, auf NewTOS nie genommen, `__mint == 0`) und TOS-Zweig `tfork(newprog, save)`; gesichert werden x19–x30, d8–d15, sp; das Kind setzt auf dem sp des Aufrufers fort (AArch64 adressiert Locals über sp, m68k über a6), der Elternteil ruft `tfork` auf einem eigenen 16-KiB-Stapel, damit das Kind den Stapel unter sp benutzen darf; Sicherungsbereich statisch wie im Original |
| `sdk/libc/newtos-arch/setstack.S` (neu) | `_setstack`: sp = newsp − 64 (wie m68k), 16-ausgerichtet; Disassembly von `startup()` geprüft: nach `_setstack` liest es nichts mehr vom Stapel |
| `sdk/libc/overlay/mintlib/thread.c` (neu) | mintlibs `thread.c` mit sechs LP64-Casts für die 32-Bit-Basepagefelder, sonst unverändert |
| `sdk/libc/overlay/posix/system.c` | **gelöscht** — `system()` ist mintlibs eigenes `posix/system.c` |
| `sdk/libc/build-libc.sh` | `thread.c` nicht mehr ausgeschlossen, `vfork.S`/`setstack.S` gebaut, Kommentare |
| `sdk/libc/tests/system-host.{sh,c}`, `sdk/run-host-tests.sh` | Hosttest auf das unveränderte `system.c` umgestellt (Shell-Probe → „keine Shell“ → TOS-Zweig mit denselben 9 Prüfungen; das Objekt trägt den POSIX-Zweig; kein Overlay mehr) |
| Doku | `docs/sdk/07-mintlib-limits.md` (§8 neu, Tabellen), `sdk/examples/port10.c` (Kommentar) |

Ausgepackter mintlib-Baum mit `MINTLIB_REFRESH=1` neu erzeugt (das gelöschte
Overlay läge sonst in `sdk/libc/mintlib/posix/system.c` weiter); ein
vorhandener alter Auspack-Stand muss genauso erneuert werden.

Prüfungen `checks/s2e-*`: Hosttests **1081/0/1**, Release **64/94** beide
Boards, Clippy **80/125**, Driftprobe **31**, SDK **13/0/0**. QEMU: alle
Programme neu gegen die neue libc gebaut (`checks/s2e-build-all.txt`; Doom
`undefined symbols: 0`, Doom/PORT10/VFORKT enthalten `system`, `vfork`,
`tfork`, `__waitval`); Desktop/TeraDesk/QED/APITEST-Bild pixelgleich
Stufe 1, Doom VERIFIED, APITEST 106/0/5, CRASHTST ×2, KBDVEC + EmuCON,
Selbsttests wie zuvor, PORT10 42/44 (jetzt mit mintlibs `system()`, TOS-Zweig);
S2ALL `s2e2`: **PFLAGS ×2, P34, MEM03, P567, TSR, OWN, VFORK PASS — „S2ALL:
PASS“**. VFORK: Kind läuft zuerst und schreibt Elternspeicher, `waitpid` →
pid + Status `42<<8`, zweites `vfork`, ST-Pool zurück, `system()` mit
`$SHELL` über den POSIX-Zweig → `127<<8` (execve ist auf TOS `Pexec(200)`,
`EINVFN` wie EmuTOS).

**Sondenkorrektur (kein Produktfehler):** VFORKT prüfte zuerst über mintlibs
`WEXITSTATUS` und bekam 0, obwohl das Statuswort stimmte (`0x2A00`).
Ursache: `<sys/wait.h>` legt `struct __wait` als big-endian-Bitfelder an
(`junk:16` zuerst) — auf little-endian AArch64 liegt `retcode` dann in Bit
16..23 (Hostgegenprobe). Die Sonde prüft jetzt das Statuswort; der
Header-Fehler ist unten gemeldet, nicht behoben.

## Pi-Karte (nur Kernel und geänderte Programme; die echte Karte nur auf Ansage)

Alle Programme müssen neu auf die Karte, weil jedes jetzt die
PRGFLAGS-Notiz trägt und die libc neu ist — die alten laufen weiter (ohne
Notiz → `PF_STANDARD`), aber nur die neuen entsprechen dem Baum:
`kernel8.img` (`cargo xtask build --board rpi4 --release`), QED.NAP,
TeraDesk DESKTOP.NAP, DOOM.NAP, APITEST.NCP, die übrigen Beispiele/Sonden aus
`bin/` (Prüfsummen `bin/SHA256SUMS`, `repo-binaries.sha256`). Die neuen
Sonden (PFLAGS1/2, P567, TSRPAR/TSRRES/TSRUSE, OWNPAR/OWNKID, VFORKT, S2ALL)
gehören nach `C:\`.

## Offen / zur Entscheidung

1. **f) PROC02 nicht gemacht.** Vererbung der Standardhandles und der
   `Fforce`-Benutzungszähler verlangen ein anderes Dateitabellenmodell
   (EmuTOS `sft[].f_use`/`f_own`, gemeinsame OFDs; NewTOS hat
   `Slot::File`/`Slot::Redirect` ohne Zähler) und das Schließen der Handles
   bei `Pterm` (`pterm.rs` nennt es „Phase 08“). Das ist mehr als ein
   Nachtrag dieser Stufe; PORT10 bleibt bei 42/44.
2. **Notizformat:** 4-Byte-Kopfworte wie `rust-lld`/`llvm-readobj` statt der
   „8-byte words“ der gepinnten gABI-Fassung (Spec nennt es). Bitte
   bestätigen.
3. **Pexec-4-Kind als Thread im Speicherraum des Aufrufers (E6 + E15):** es
   darf jeden Block des Aufrufers freigeben (FreeMiNT `FORK_SHAREVM`), und
   mintlibs `tfork` lässt dadurch — wie unter FreeMiNT — die Umgebung der
   `Pexec`-5-Basepage bis zum `Pterm` des Aufrufers liegen und gibt danach
   die eigene Umgebung des Aufrufers frei (mintlib-Code, unverändert). So
   gebaut, wie der Plan es formuliert; bitte bestätigen.
4. **TSR-Speicher bleibt auf der Belegtliste** (FreeMiNT `TSR_Q`) statt wie
   in EmuTOS aus beiden Listen zu fallen — in `COMPATIBILITY.md` benannt.
5. **Stapel beim Start über `Pexec` 4/6 immer aus `p_hitpa`** (EmuTOS
   `proc_go`), auch für `PE_LOAD`-Programme; vorher nahm der Kernel die
   ursprüngliche TPA-Obergrenze, das SDK-`crt0` setzte ohnehin `p_hitpa`.
6. **Nicht übernommen aus Plan Stufe 2:** `Mxalloc`-Moduskodierung
   (Schutznibble, `F_KEEP`/`M_KEEP`) — nicht Teil der Teilschritte a–e;
   `M_KEEP`-Blöcke („jeder darf freigeben“) gibt es daher noch nicht.

## Beobachtungen außerhalb des Auftrags (unverändert)

- mintlibs `<sys/wait.h>`: `WIFEXITED`/`WEXITSTATUS`/`WTERMSIG` lesen über
  big-endian-Bitfelder und sind auf AArch64 falsch (`WEXITSTATUS` immer 0) —
  Kandidat für ein Overlay (Header-Portierung, Muster B11).
- `selftest_sleepwake_switch` scheitert weiter mit denselben 5 Prüfungen.
- `kernel/src/accs.rs` `free_accs`-Kommentar (aus Stufe 1) nicht angefasst.
- `populate_and_map` gibt bei einem Fehler nach der Umgebungszuteilung nur
  den TPA frei, nicht die Umgebung (vorbestehend; der neue
  `create_basepage` gibt beide frei).
