# ARCH03 Stufe 3c — Quittung

**Status: UMGESETZT bis auf einen Punkt, der eine Entscheidung braucht
(S2ALL/TSRPAR unter Schutz an/strict) · in QEMU auf beiden Boards in allen
drei Modi belegt · Abnahme durch den Tester offen · Pi offen** · 2026-10-05 ·
Zweig `arch03`, Basis HEAD `884111c`, Baum vorher sauber, nichts committet ·
Belege [measurements/measurement-2026-10-05-arch03-3c/](../measurements/measurement-2026-10-05-arch03-3c/)
(`RUNINFO.TXT`, `checks/`, `serial/`, `screendumps/`, `method/`, `bin/`).

Der Auftrag nannte den Dateinamen `…-3c-STATUS-DONE.MD`; die Datei heißt
`UNFINISHED`, weil die geforderte Prüfung „S2ALL PASS in allen drei Modi“
nicht erreicht ist (Punkt 2 unten). Aus demselben Grund ist
`ARCH03-3b-STATUS-UNFINISHED.MD` **nicht** umbenannt (dort ein Vermerk).

## Punkte des Auftrags

| | Stand |
|---|---|
| 1 E10 (a): fremde `G_USERDEF`-Routine im laufenden Prozess, beide Modi, mit `grant_temp`/`revoke_temp` | **fertig**, Zwei-Prozess-Sonde PASS in ON/OFF/STRICT, raspi4b + virt |
| 2 Lader-Reihenfolge wie EmuTOS (Umgebung vor TPA) | **gebaut**; die erwartete Wirkung auf S2ALL **tritt nicht ein** — Ursache ist E9 + E16, nicht die Reihenfolge (gemessen) |
| 3 E6 vs. strict dokumentieren | **fertig** (`4b-pexec.md`, `COMPATIBILITY.md`, Plan) |
| 4 `F_KEEP` nach FreeMiNT; Tabellenvorrat erschöpft → Meldung + Zähler | **fertig** (Hosttests; zur Laufzeit nicht ausgelöst) |
| Plan „Stufe 3 wie gebaut“, `TASK/ARCH03.md` Fortschritt | **fertig** |

## Was getan wurde

| Bereich | Datei / Funktion |
|---|---|
| E10 | `tos-api/src/aes/object.rs` `ob_user`: A97-Rückgabe entfernt; Callout im laufenden Slot; Eigentümer ≠ laufender Prozess → `memprot::grant_temp(slot, owner)` vor, `revoke_temp(slot)` nach `block_for_userdef`; toter Eigentümer-Slot → Zustand unverändert. Kommentare in `object.rs`, `window.rs`, `vdi/mouse.rs` nachgezogen |
| Lader | `tos-api/src/bdos/process/loader.rs` `load_from_source`: `alloc_env_for` vor `arena::alloc_tpa`, Umgebung wird bei TPA-Fehler und Ladefehler wieder freigegeben, `set_owner` für beide nach der TPA; `ProcessSetup.env_addr`; `alloc_env`/`alloc_env_for` ohne Besitzer-Parameter |
| `F_KEEP` | `tos-types/src/memory.rs` `F_KEEP = 0x4000`; `bdos/mem/traps.rs` `mxalloc` markiert, `kernel_owner()`; `bdos/mem/region.rs` `mark_keep/is_kept/release_kept`, `forget` löscht das Flag, `detach_all` gibt den letzten Link an den Kernel; `gemdos/memory.rs` `release_as_caller`: Nicht-Besitzer darf einen geerbten `F_KEEP`-Block freigeben |
| Tabellenmangel | `bdos/process/memprot.rs` `note_shortfall`, `table_shortfalls_of`, `ShortfallReport`, `take_shortfall_report` (einmal je Prozess, beim Freigeben des Slots zurückgesetzt); `kernel/src/arch/aarch64/exceptions.rs` `rust_sync_handler` druckt `[memprot] page-table arena exhausted: process slot … (PD …) keeps non-owner rights in … region(s); shortfalls since boot …` |
| Doku | `docs/api/aes/window-graphics.md` (WF_NEWDESK-Notiz), `docs/api/gemdos/process/4b-pexec.md` (Reihenfolge, strict/E6, E10), `docs/api/gemdos/memory/44-mxalloc.md` + `49-mfree.md` (`F_KEEP`), `docs/COMPATIBILITY.md`, Plan-Abschnitt „Stufe 3 wie gebaut“, `TASK/ARCH03.md` |
| Tests | `tos-api/tests/arch03_3c_keep.rs` (3), `arch03_3c_e10.rs` (2), `arch03_3c_table_shortfall.rs` (1) |
| Sonde | `sdk/examples/e10usr.c` + `e10kid.c` (Varianten E10USR/E10USRS/E10USRO) |

## Quellen

- EmuTOS (`references/emutos`, Pin `1a7484ab`): `aes/gemoblib.c:411-426`
  (`call_usercode`: `jsr (a0)` auf dem laufenden Stapel), `:433-448`
  (`ob_user`), `:792-793` (`just_draw` → `ob_user`); `bdos/proc.c:281-287`
  (`alloc_env` zuerst), `:289-299` (`alloc_tpa`, Umgebung bei Fehler frei),
  `:302-307` (`set_owner`); `bdos/umem.c:241` (`mode &= MX_MODEMASK` — EmuTOS
  ignoriert `F_KEEP`).
- FreeMiNT (`references/freemint`, Pin `f906416`): `sys/arch/mprot030.c:775-805`
  (`prot_temp`); `sys/mint/mem.h:104-105` (`F_KEEP`), `:45` (`M_KEEP`);
  `sys/dosmem.c:131-135` (Mxalloc setzt `M_KEEP`), `:161-165` (Semantik),
  `:237-242` (nicht Teil des Modus), `:365-398` (jeder darf geerbten
  `M_KEEP`-Block freigeben); `sys/proc_help.c:242-258` (beim Ende an
  `rootproc`); `sys/k_exec.c` Modusschalter (Modus 6 = kein `FORK_SHAREVM`,
  4 = Thread).

## Prüfungen (Einzelheiten `RUNINFO.TXT`)

| | 3b (c5) | 3c (c1, c2) |
|---|---|---|
| Hosttests | 1123/0/1 | **1129/0/1** (+6) |
| Release frisch nach `cargo clean -p kernel -p tos-api …`, tos-api/kernel, beide Boards | 64/94 | **64/94** |
| Clippy tos-api/kernel, rpi4 und qemu-virt | 76/125 | **76/125** |
| Driftprobe | 31 | **31** |
| SDK-Hosttests | 13/0/0 | **13/0/0** |

QEMU c1, **ON, OFF und STRICT je die volle Serie** (16 Läufe + desk3a, alle
rc=0): Desktop, TeraDesk, QED, APITEST-Bilder bytegleich mit 3b (ON/OFF) bzw.
mit 3c-ON (STRICT); APITEST 106/0/5; Doom VERIFIED; Accessory-Start;
PORT10 42/44 (PROC02 wie vorher); CRASHTST, UACC3A (26), MPROT3B, KBDVEC +
EmuCON PASS; virt: CRASHTST/UACC3A/MPROT3B/E10USR PASS, Selbsttests wie 3b
(sleepwake dieselben 5 FAIL), Desktop; desk3a in allen drei Modi
untereinander bytegleich, gegen 3b nur Dateizeit/Dateizahl. **E10USR PASS in
allen Modi auf beiden Boards**; derselbe Sonde auf dem 3b-Kernel: FAIL (0
Aufrufe im Kind) — die Sonde unterscheidet. **S2ALL:** OFF PASS; ON FAIL
(TSRPAR, eine Prüfung); STRICT FAIL (P567 Fall 6, TSRPAR).

## Offen / zur Entscheidung

1. **S2ALL/TSRPAR „TPA beyond blkln back in the pool“ unter ON (und
   STRICT).** Die Lader-Reihenfolge ist jetzt die von EmuTOS (die Umgebung
   liegt unter der TPA, gemessen), die Prüfung scheitert aber gleich wie in
   3b (TT größter Block `0x39f78000 → 0x39b76000`). Diagnose mit einer
   TSRPAR-Variante, die die Adressen ausgibt: TSRRES' `Mxalloc(64)`-Block
   liegt bei `0x02289000` = **direkt hinter** seiner 4-MiB-TPA
   (`0x01e89000`); der von `Ptermres` zurückgegebene Rest ist dadurch ein
   eigener freier Block. Unter OFF fallen Umgebung und kleiner Block in ein
   Loch darunter (16-Byte-Raster) → PASS. **Ursache: E9 (feste 4-MiB-TPA statt
   EmuTOS' größtem Block) + E16 (Seitenraster) + first fit**, nicht die
   Reihenfolge (die 3b-Vermutung war falsch, und die Leitvorgabe beruhte
   darauf). Ich habe weder die Prüfung noch die Vergabestrategie geändert.
   Möglichkeiten für den Lead: (a) die Prüfung ist ein Layout-Stellvertreter —
   durch eine semantische ersetzen (z. B. „der Rest ist ein freier Block an
   `PD+blkln`“); (b) TPA aus dem oberen Ende des größten freien Blocks
   (Strategieänderung, nicht EmuTOS); (c) E9 neu betrachten (TPA = größter
   Block wie EmuTOS `alloc_tpa`).
2. **S2ALL unter STRICT** scheitert zusätzlich in P567 Fall 6 (das
   `Pexec`-6-Kind führt Code aus dem `PROT_P`-Text des Elternteils aus) und in
   TSRPAR (TSRRES schreibt in das `PROT_P`-Postfach des Elternteils). Beides
   sind `Pexec`-0/6-Kinder mit Zugriff auf privaten Elternspeicher — genau
   das, was `memprot=strict` verbietet (FreeMiNT `MPF_STRICT`; Modus 6 ist dort
   kein Thread). Das ist korrektes strict-Verhalten, kein Fehler; „S2ALL PASS
   in strict“ ist mit diesen Stufe-2-Proben nicht erreichbar, ohne sie für
   strict zu ändern (wie MPROT3B es mit Varianten tut). Entscheidung des
   Leads.
3. Die Leitvorgabe nennt „strict gilt nur für `Pexec`-0-Kinder“; gebaut und
   dokumentiert ist „für Kinder mit eigenem Raum: `Pexec` 0 **und 6**“ (6 ist
   in FreeMiNT kein Thread, `k_exec.c`: `thread = (mode == 4)`). Offen
   gelegt; bei Bedarf Spec-Satz anpassen.
4. `F_KEEP` wirkt in jedem Schutzmodus (FreeMiNT: Regionsbuchhaltung, auch
   bei `MEM_PROT=NO`) — damit ändert sich `memprot=off` gegenüber 3b für
   Programme, die `0x4000` setzen (bisher bekannt: keines).
5. Tabellenmangel: gebaut ist „Nicht-Besitzer-Rechte + Meldung“, nicht
   Plan-Frage 14 „`ENSMEM` der auslösenden Allokation“ — Abweichung im Plan
   vermerkt.

## Für den Tester

- Karten: `method/seed-card.sh` (nie `tools/make-sd-image.sh`), Serie
  `method/runs-3c.sh <label> <cards> on|off|strict` (volle Serie in jedem
  Modus), desk3a `method/desk3a-run.sh <label> <scratch> <mode>`.
- Neue Sonde: Karte mit `#Z` `C:\E10USR.NAP` (ON) / `E10USRS.NAP` (STRICT)
  / `E10USRO.NAP` (OFF), Crash-Kernel `bin/kernel8-crash-c1.img` (der
  Injektor beantwortet den strict-Fall r); virt: `bin/kernel-virt-crash-c1.img`
  (wählt die Variante selbst, startet E10 nach `go_graphic`). Erwartung
  „E10USR: PASS“; STRICT genau ein Absturz („private memory, read“).
  Endbild: grünes Muster des `WF_NEWDESK`-Baums, Kasten mit Kreuz.
- Pi: `bin/kernel8-c2.img`; `cmdline.txt` mit `memprot=off` bzw.
  `memprot=strict` schaltet um. Pi-Lauf nur auf Ansage.

## Beobachtungen außerhalb des Auftrags (unverändert)

- `evnt_timer` in QEMU raspi4b: ein einzelnes `evnt_timer(4000)` hielt den
  Bildschirm in Vorversuchen sichtbar kürzer als 4 s; nicht untersucht (die
  Sonde hält jetzt 3 × 4 s).
- `selftest_sleepwake_switch` 5 FAIL vorbestehend.
- Doom-Einzelbilder zeitabhängig, wie in 3b.
