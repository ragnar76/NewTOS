# Desktop im User-Space — Plan v2, gegen den Baum von heute nachgemessen

> Stand: **2026-09-26** · Branch `desk-rebuild` · HEAD `0d5042b`
> Auftrag: `TASK/DESK43.md` (P1). **Nur Analyse + Dokument** — `kernel/`,
> `tos-api/`, `tos-types/`, `docs/STATUS.md`, `TASK/DESK43.md` sind in diesem
> Job unberührt geblieben.
> Vorgänger: `docs/analysis/desktop-userspace-plan.md` (2026-09-01, 1258 Z.).
> Der bleibt liegen — er ist das Protokoll dessen, was am 2026-09-01 gedacht
> wurde, und ein Teil davon ist inzwischen **gebaut**, nicht verworfen.

Zwischen dem alten Plan und heute liegen **412 Commits**, davon **38 an
`kernel/src/desk/`** (`git log --since=2026-09-01`). Der alte Plan beschreibt
Code, der so nicht mehr aussieht. Dieses Dokument misst neu, sagt Abschnitt für
Abschnitt, was daran heute falsch ist, und schneidet den Rest in zehn
Arbeitseinheiten.

**Marken:** **[M]** = in diesem Job an Baum/Quelle gemessen, mit `file:line`.
**[A]** = Annahme, als solche gekennzeichnet. **[?]** = nicht ermittelbar.

---

## 0. Das Ergebnis in sechs Sätzen

1. **[M]** Von den neun Stufen des alten Plans sind **S0, S1a–S1d, S3 und S7
   gebaut**; **S2, S4, S5, S6, S8 stehen aus**.
2. **[M]** Der gemessene Ausgangspunkt ist nicht mehr gültig: `kernel/src/desk/`
   ist **19 133 Zeilen in 19 Dateien** (nicht 9 234/15), aber die Zahl der
   Umgehungen ist von 56 Aufrufstellen auf **drei `tos_api::`-Symbolgruppen und
   sieben `crate::`-Symbolgruppen** gefallen.
3. **[M]** Die Wurzel ist heute **eine** Sache und hat einen Namen: AES-Lücke
   **G1** — `desk::run()` kehrt nie zurück, also kommt `sh_main` nie wieder an
   die Reihe (`kernel/src/shell_loop.rs:430`).
4. Der Umzug zerfällt in **zwei unabhängige Achsen**, die der alte Plan in
   Stufe S6 gebündelt hat: das **Prozessmodell** (der Desktop stirbt und wird
   von `sh_main` neu gestartet) und die **Verlagerung nach EL0**. Achse A
   braucht **kein** EL0 und ist durch A106 bereits erwiesen. Die Bündelung in
   S6 war ein Fehler und wird hier aufgelöst (§4).
5. **[M]** Die größte offene Frage des alten Plans ist beantwortet: der Desktop
   passt in die TPA. `kernel::desk::*` belegt im Release-Kern **233 928 Byte
   `.text`** von 697 160 gesamt (`llvm-nm --print-size`, §2.5) — bei
   `TPA_SIZE = 4 MiB` (`tos-api/src/bdos/process/arena.rs:52`) kein Thema.
6. **[M]** Zur wichtigsten Randbedingung — was den dritten Abdrift verhindert —
   sagt die Quellenlage mehr, als der alte Plan wusste, und etwas anderes, als
   vermutet wird (§6). Der Satz, auf den es ankommt, steht in `343b3c5`.

---

## 1. Der alte Plan, Abschnitt für Abschnitt

| § (alt) | Aussage von 2026-09-01 | Heute | Beleg |
|---|---|---|---|
| §1.1 | `desk/` = 9234 Z. / 15 Dateien | **falsch** — 19 133 Z. / 19 Dateien | `wc -l kernel/src/desk/*.rs` |
| §1.1 | Start aus `kernel/src/lib.rs:329` (`desk::run()`) | **falsch** — `lib.rs` hat eine AES-Sessionschleife; `desk::run()` hängt an `shell_loop.rs:430` im `DESKTOP_APP`-Arm | `kernel/src/lib.rs:416-487`, `kernel/src/shell_loop.rs:418-437` |
| §1.1 | 22 AES-Opcodes über `svc #2` | **überholt, nach oben** — 33 Opcodes | `kernel/src/desk/mod.rs:57-89` |
| §1.2/1 | Kontextname bei `mod.rs:169` | **falsch** — die Formulierung „kernel-boot context" steht nicht mehr in `mod.rs`; die Selbstbeschreibung liegt jetzt in `dosbind.rs:50-52` | `kernel/src/desk/dosbind.rs:50` |
| §1.2/3 | 56 Call-Sites über 21 `tos_api`-Symbole | **überholt** — 3 Symbolgruppen, siehe §2.2 | §2.2 |
| §1.2/4 | `ap_private` kommt aus `rs_trees[DESKTOP][ROOT].ob_spec`, D-R1-6 sofort behebbar | **richtig und gebaut** | `tos-api/src/aes/init.rs:61-65,98`, `appl.rs:303` |
| §2 (ganz) | EmuTOS-Prozessmodell aus der Quelle | **gilt unverändert** und ist inzwischen als Spec abgelegt | `docs/api/aes/shell-loop.md` (1109 Z.) |
| §3.2 | Zehn fehlende Bausteine für den Shell-Loop | **neun davon gebaut** — `sh_main`, `sh_ldapp` (4 Arme), `sh_chdef`, `sh_chgrf`/`sh_toalpha`/`sh_tographic`, `sh_draw`, `process_inf2` inkl. `#Z`, `wm_new`, `p_setappdir`; **P6-005 aufgelöst**, `pending_launch()` existiert nicht mehr | `kernel/src/shell_loop.rs`, `tos-api/src/aes/init.rs:442`, `tos-api/src/aes/shell.rs` |
| §3.2 **[STOP-1]** | Schichtfrage Kernel vs. `tos-api` | **entschieden und gebaut**: Zustand/Policy in `tos-api/src/aes/shell.rs`, Schleife + EL0-Eintritt in `kernel/src/shell_loop.rs` — genau die Empfehlung | `kernel/src/shell_loop.rs:14-21` |
| §3.3 | `programs/`-Workspace, `newtos-gem`, `NEWDESK.NAP` | **unverändert offen** — `programs/` existiert nicht, `Cargo.toml:11-13` schließt es weiter ins Leere aus, `cargo xtask build-programs` gibt es nicht (`tools/xtask/src/main.rs:41-46`) | gemessen |
| §3.3 **[A]** | „passt in die 4-MiB-TPA" ungemessen | **gemessen**, §2.5 | `llvm-nm` |
| §3.4 (a) | AES-`DESKTOP`-Baum gehört in diese Arbeit | **gebaut** | `tos-api/src/aes/init.rs:98` |
| §3.4 (b) | Desktop-eigene `.NRSC` gehört **nicht** hierher | **gilt**, und DESK42 hat inzwischen Dialoge als EmuTOS-Ressourcenbäume nachgebaut, ohne die EL0-Frage zu berühren — die Trennung war richtig | `TASK/DESK42-STATUS-DONE.MD` |
| §3.5 | Fünf Server-Korrekturen S1a–S1d + S7 | **alle fünf gebaut**, siehe §3 | §3 |
| §3.5 | `save_windows`/`restore_windows` löschen | **halb** — kein Produktivaufrufer mehr, aber die Funktionen stehen noch in `tos-api/src/aes/window.rs:1419,1444` und zwei Integrationstests schreiben ihre Aufrufreihenfolge als Vertrag fest | `tos-api/tests/aes_app_return_descriptor.rs:332,341`, `aes_launch_boundary_reset.rs:337,344` |
| §3.6 | Datei-für-Datei-Schicksal der 9234 Z. | **veraltet** — die Tabelle nennt Dateien mit falschen Zeilenzahlen und kennt `dosbind.rs`, `vdibind.rs`, `pro.rs`, `ins.rs` nicht | §2.1 |
| §3.6 | Kernel-Restposten `launch.rs:137` (`ftype`) | **richtig, aber unvollständig** — es sind **zwei** Stellen | `kernel/src/launch.rs:266`, `kernel/src/accs.rs:48` |
| §3.6 | Tote Selftests `dir.rs:1086`, `inf.rs:1388`, `inf.rs:1477` | **richtig und schlimmer** — es sind **vier**, alle weiter ohne Aufrufer | `dir.rs:2557`, `inf.rs:4182`, `inf.rs:4289`, `ins.rs:1130`; `grep -rn 'desk::dir::selftest\|desk::inf::selftest\|desk::ins::selftest' kernel/src` = 0 Treffer |
| §3.7 (1) | `gsx_*`: unser Desktop sei „reiner als EmuTOS" | **überholt** — seit `vdibind.rs` öffnet der Desktop eine **eigene** virtuelle Workstation und ruft `v_pline`/`vs_clip`/`vswr_mode` über den VDI-Trap. Das ist nicht reiner, sondern *app-fähiger* — und macht G5 billiger (§5, U7) | `kernel/src/desk/vdibind.rs:1-31,140` |
| §3.7 (2) | `Keytbl` liefert einen Zeiger, den EL0 nicht dereferenzieren kann | **gelöst und gebaut** — `KEYTBL_AREA_VA` in der TPA-Low-Region | `tos-api/src/xbios/keytbl.rs:25,60-74` |
| §3.7 (3) | globales `CUR_DTA` wird zur Zeigerleiche | **gelöst** — `p_xdta` pro Prozess, mit **benanntem** `KERNEL_DTA`-Rückfall genau für den EL1-Desktop, dessen Entfernung dort schon auf diesen Plan verwiesen wird | `tos-api/src/bdos/fs/find.rs:52-66,86-105` |
| §4 | „S1–S4 fassen den Boot-Pfad nicht an, S6 ist der Umschalter" | **die Bündelung in S6 ist der Fehler des alten Plans** — §4 |  |
| §5 (ganz) | 56 Call-Sites, 15 versteckte Arbeitspakete | **überholt** — 12 der 15 sind erledigt, §3 | §3 |
| §5.4/19 **[STOP-3]** | `console_pause_for_key` streichen | **unentschieden, Code steht** | `kernel/src/desk/main.rs:1243-1249` |
| §5.5/20 | `aes::window::redraw()` auflösen | **gebaut** — `form_dial(FMD_FINISH)`, Opcode 51 | `docs/analysis/desk-aes-gaps-2026-09-14.md` §1.1 |
| §7 | Was den dritten Abdrift verhindert | **teils überholt, teils widerlegt** — §6 |  |
| §8 S0 | `docs/api/aes/shell-loop.md` | **geliefert**, 1109 Zeilen | gemessen |
| §9/1 | TPA-Passung ungemessen | **gemessen** | §2.5 |
| §9/3 | `evnt_multi` aus einem *dauerhaft* darin sitzenden EL0-Client | **weiter offen** und inzwischen **verschärft** — ACC10 | `TASK/ACC10.md` |
| §9/5 **[STOP-1]** | Schichtfrage | entschieden, s.o. |  |
| §11 | Entwurf ADR-023 | **Nummer ist vergeben.** `docs/MEMORY.md:1583` trägt ADR-023 („PROT_*"). Der Desktop-ADR braucht eine freie Nummer | `docs/MEMORY.md:1583` |

### 1.1 Was am alten Plan sachlich falsch war (nicht bloß veraltet)

1. **§3.7 (1), „unser Desktop ist an dieser Stelle reiner als EmuTOS".** Der
   Satz war zum Zeitpunkt richtig und ist heute unzutreffend, aber er trug
   schon damals einen Denkfehler: „reiner" war ein Lob für etwas, das in
   Wahrheit eine *fehlende* Fähigkeit war (der Desktop konnte keine
   Linie zeichnen). `vdibind.rs` hat das mit einer eigenen `v_opnvwk`-
   Workstation gelöst — dem Weg, den jede GEM-Anwendung geht. Die Lehre gehört
   in den Plan: „wir tun es gar nicht erst" ist kein Konformitätsargument.
2. **§5.3/17, `vdi::mouse::display_at` „entfällt ersatzlos, die Positionierung
   wandert in den Shell-Loop".** Die Messung vom 2026-09-14 zeigt: sie musste
   nirgendwohin wandern. Der Zeiger steht auch ohne den Aufruf in der Mitte,
   weil `tos-api/src/vdi/mouse.rs::ensure_init` dasselbe tut wie EmuTOS' VDI
   (`vdi_mouse.c:582-583`). Der Plan hatte die Arbeit vorhergesagt, die es
   nicht gab.
3. **§11, ADR-Nummer.** ADR-023 ist belegt.

---

## 2. Der Ausgangspunkt, neu gemessen

### 2.1 Umfang

**[M]** `kernel/src/desk/` = **19 133 Zeilen in 19 Dateien**, davon **10 443
Zeilen** ohne Kommentar- und Leerzeilen.

| Datei | Zeilen | | Datei | Zeilen |
|---|---:|---|---|---:|
| `inf.rs` | 4366 | | `insapp.rs` | 420 |
| `main.rs` | 3476 | | `icons.rs` | 360 |
| `dir.rs` | 2669 | | `act.rs` | 312 |
| `ins.rs` | 1262 | | `app.rs` | 302 |
| `menu.rs` | 1196 | | `redraw.rs` | 264 |
| `win.rs` | 1030 | | `vdibind.rs` | 244 |
| `mod.rs` | 1001 | | `pro.rs` | 133 |
| `global.rs` | 884 | | `dlg.rs` | 127 |
| `model.rs` | 573 | | `ftype.rs` | 87 |
| `dosbind.rs` | 427 | | | |

Neu gegenüber 2026-09-01: `dosbind.rs` (GEMDOS-Trapbindung), `vdibind.rs`
(VDI-Trapbindung), `pro.rs` (`deskpro.c`), `ins.rs` (`deskins.c`).
Das Wachstum ist ganz überwiegend **Funktionalität** (DESK01–DESK42) und
**Belegkommentar**; der Kommentaranteil liegt bei 45 %.

### 2.2 Das Bypass-Register — jede Stelle, heute

`/usr/bin/grep -n 'tos_api::' kernel/src/desk/*.rs` liefert **36 Treffer**,
davon **19 reine Doc-Kommentare**. Was im Code steht:

**A — `tos_api::aes::object::{Object, TedInfo, IconBlk}` · 11 `use`-Zeilen**

| Symbol | Dateien | Nutzungen |
|---|---|---:|
| `Object` | `dir.rs:53`, `dlg.rs:21`, `global.rs:16`, `inf.rs:29`, `ins.rs:31`, `insapp.rs:32`, `menu.rs:38`, + `dir.rs:1623` (Cast) | 51 |
| `IconBlk` | `app.rs:18`, `icons.rs:19`, `ins.rs:31`, `model.rs:23`, `win.rs:16` | 17 |
| `TedInfo` | `dir.rs:53`, `dlg.rs:21`, `inf.rs:29`, `ins.rs:31`, `insapp.rs:32` | 22 |

**Urteil: kein Befund, sondern eine Umzugsfrage.** `OBJECT`/`TEDINFO`/`ICONBLK`
sind `include/obdefs.h`, die öffentliche GEM-Schnittstelle, die EmuTOS' eigener
Desktop durchgehend dereferenziert (`deskapp.c`, `deskobj.c`, `deskwin.c`). Das
ist ausdrücklich so in `docs/analysis/desk-aes-gaps-2026-09-14.md` §1.2
festgehalten, gegen ein Briefing, das sie für einen Befund hielt. Sie ziehen
nach `tos-types`, weil sie dort hingehören — nicht, weil ihr Aufruf falsch wäre.

**B — `tos_api::vdi::font::{CELL_W, CELL_H, SMALL_CELL_W, SMALL_CELL_H}` · 8 `use`-Zeilen**

| Symbol | Dateien | Nutzungen |
|---|---|---:|
| `CELL_W` | `act.rs:22`, `app.rs:19`, `dir.rs:54`, `inf.rs:30`, `ins.rs:32`, `menu.rs:31`, `win.rs:17` | 96 |
| `CELL_H` | `app.rs:19`, `dir.rs:54`, `inf.rs:30`, `ins.rs:32`, `menu.rs:31`, `win.rs:17` | 104 |
| `SMALL_CELL_W` | `app.rs:19`, `icons.rs:20` | 5 |
| `SMALL_CELL_H` | `app.rs:19`, `icons.rs:20` | 6 |

**Urteil: zwei echte Umgehungen, AES-Lücken G4 und G5.**
`CELL_W`/`CELL_H` sind `gl_wchar`/`gl_hchar` und kommen bei EmuTOS aus
`graf_handle` (`deskmain.c:1956`). Der Opcode existiert, liefert exakt dieselben
8/16 (`tos-api/src/aes/graf.rs` `graf_handle` → `gl_wchar()`/`gl_hchar()`;
`tos-api/src/vdi/font.rs:19,21`), und die Bindung liegt schon im Desktop
(`kernel/src/desk/mod.rs:501`) — sie wird nur für `v_opnvwk` benutzt
(`vdibind.rs:140`), nicht für die Metrik. **Verhaltensgleich, reine Umstellung.**
`SMALL_CELL_W/H` sind `gl_wschar`/`gl_hschar` und haben **keine** AES-Bindung;
EmuTOS liest dort die AES-Globale direkt (`deskapp.c:519-520`). Der Weg einer
Anwendung ist `v_opnvwk` + `vst_height` (`gemgraf.c:407`) — und seit
`vdibind.rs` hat der Desktop diese Workstation bereits offen.

**C — `tos_api::host::host()` · 1 Aufrufstelle**

`kernel/src/desk/main.rs:1247` — `h.console_read_key_blocking()` in
`console_pause_for_key`. **Echte Umgehung, ohne AES-/GEMDOS-Äquivalent, und
ohne EmuTOS-Gegenstück** (`sh_chgrf` schaltet direkt zurück,
`gemshlib.c:513-523`). Das ist **[STOP-3]** des alten Plans, unverändert offen.

**D — `crate::*` · 7 Symbolgruppen**

| Symbol | Aufrufstellen | Ersatz |
|---|---|---|
| `crate::shell_loop::{sh_alpha_enter, sh_alpha_leave, sh_gem_show, sh_gem_enter, sh_gem_leave, sh_run_pending_console}` | `main.rs:1021,1031,1089,1141,1170`, `mod.rs:970` | **entfällt komplett mit G1** — das sind Schritte aus `sh_main`, die der Desktop selbst zieht, weil er nicht zurückkehren kann |
| `crate::launch::run_program_with_tail` | `main.rs:1022,1090` | `pro::pro_run` → `shel_write` (**existiert schon**, `pro.rs:121`) |
| `crate::keymap::to_ascii` | `main.rs:1478` | XBIOS `Keytbl` (0x10) — **existiert**, `tos-api/src/xbios/keytbl.rs`; EmuTOS liest `KEYTAB.shift[]`, `deskmain.c:1144-1145` |
| `crate::fbcon::write_byte` | `main.rs:1245` | entfällt mit `console_pause_for_key` (STOP-3), sonst `Cconws` |
| `crate::initinfo::{VERSION_STRING, machine_name, INDEPENDENCE_NOTICE, AUTHORSHIP_LINE}` | `inf.rs:2339,2341,2345,2358` | eigene Konstanten des Desktop-Crates bzw. OSHEADER/`_MCH`-Cookie (beides liegt per Prozess in der TPA, `loader.rs`) |
| `crate::sync::SpinMutex` | `global.rs:18`, `inf.rs:34` | Single-Thread-EL0: `RefCell`/`Cell` |
| `crate::kprintln` | `mod.rs:44`, `dir.rs:2558`, `inf.rs:4183,4290`, `ins.rs:1131` | `Cconws`; die vier hinter den letzten vier Stellen liegenden Selftests werden EL0-Testprogramm |

**E — Kernel hängt an `desk::` · 2 Aufrufstellen**

`kernel/src/launch.rs:266` und `kernel/src/accs.rs:48` benutzen
`crate::desk::ftype::{classify, FileType, Flavor}` für den AUTO-Ordner bzw. die
`.NAC`-Suche. **1:1-Ersatz ist ein eigener Endungstest im Kernel** — EmuTOS'
`autoexec` hat seinen eigenen Filter `Fsfirst("\\AUTO\\*.PRG", 7)`
(`bios/bios.c:713-714`), völlig unabhängig von `desk/`.

### 2.3 Was aus den AES-Lücken vom 2026-09-14 geworden ist

| | Lücke | Stand heute | Beleg |
|---|---|---|---|
| **G1** | Der Desktop kann kein Programm über die AES starten — `desk::run()` kehrt nie zurück | **OFFEN. Das ist dieses Ticket.** | `kernel/src/shell_loop.rs:430`, `kernel/src/desk/main.rs:1053-1075` |
| G2 | kein `cnx_put`/`cnx_get` | geschlossen (DESK09/DESK35) | `kernel/src/desk/main.rs:1100-1130,1234` |
| G3 | keine AES-Bindung setzt den Zeiger | geschlossen (entfällt) | `kernel/src/desk/mod.rs:928-936` |
| **G4** | `gl_wchar`/`gl_hchar` nicht aus `graf_handle` | **OFFEN** | §2.2 B |
| **G5** | `gl_wschar`/`gl_hschar` ohne AES-Bindung | **OFFEN**, aber billiger geworden | §2.2 B |
| G6 | GEMDOS/BIOS/XBIOS als Rust-Funktion statt Trap | geschlossen (DESK10) bis auf `keymap`/`fbcon`/`host` | `kernel/src/desk/dosbind.rs` |
| G7 | keine `form_center`/`form_dial` | geschlossen (DESK08) | `TASK/DESK08-STATUS-DONE.MD` |

### 2.4 Was `desk::run()` heute tatsächlich tut — und warum das die Lage ändert

Der alte Plan und die Kommentare im Baum sagen beide, `desk::run()` „kann nicht
terminieren und neu gestartet werden". **[M] Das stimmt so nicht mehr, und der
Unterschied ist der wichtigste Einzelbefund dieses Jobs.**

* `desk::run()` **hat** einen Rückweg: die Schleife bei
  `kernel/src/desk/mod.rs:968-978` bricht ab, wenn kein Konsolenstart ansteht,
  und ruft dann `appl_exit()` (`:983`). `sh_ldapp`s DESKTOP-Arm gibt danach 0
  zurück (`shell_loop.rs:436`) und `sh_main` macht seinen nächsten Durchlauf.
* `desk::reset()` existiert (`kernel/src/desk/mod.rs:995-1001`) und räumt
  `global`, `inf` und die VDI-Workstation komplett ab; `global::reset()` setzt
  den Zustand auf `None` (`global.rs:859-861`).
* Dieses Paar aus Abbau und Neustart ist **gefahren und abgenommen**: A106
  („AES restart") startet eine ganze Session neu — `tos_api::aes::reinit();
  desk::reset();` (`kernel/src/lib.rs:474-475`) — und der Nachweis lautet
  *„QEMU: zwei Neustarts, Kaltstart-Bild 0 px"* (`TASK/A106-STATUS-DONE.MD:1`).

**Folge:** der Desktop kann sterben und pixelgleich wiederkommen. Was bis heute
fehlt, ist nicht der Mechanismus, sondern **der Zustandskanal**: EmuTOS trägt
den Desktop-Zustand über seinen eigenen Tod hinweg in den AES-Shell-Puffer
(`app_save(FALSE)` → `shel_put`, `app_start` → `shel_get`,
`deskapp.c:832,1293`). Unser `cnx_put`/`cnx_get` legt ihn in `G.g_cnxsave` — was
nur trägt, solange der Desktop lebt. Die Opcodes 122/123 sind vorhanden
(`tos-api/src/aes/mod.rs:837-838`), der Desktop bindet sie nur nicht
(33 Opcodes, `mod.rs:57-89`, ohne 122/123).

**Das macht Achse A (§4) zu einer kleinen, geordneten Änderung statt zu einem
Sprung.**

### 2.5 Passt der Desktop in die TPA? — gemessen

`llvm-nm --print-size --demangle` auf `target/aarch64-unknown-none-softfloat/release/kernel`
(vorhandenes Artefakt, nichts gebaut), Summe über alle Symbole mit
`kernel::desk::` im Namen:

| | Byte | |
|---|---:|---|
| `.text` (`t` + `T`) | **233 928** | ≈ 228 KiB |
| `.data` (`d` + `D`) | 8 190 | |
| `.rodata` (`r` + `R`) | 1 923 | |
| *Kern gesamt `.text`* | 697 160 | zum Vergleich |
| *Debug-Profil, dieselbe Summe* | 245 060 | `opt-level = 1`, kaum Unterschied |

Dazu die Objekt-Arena: `INITIAL_ITEM_SLOTS = (NUM_WNODES + 1) × MAX_WIN_ITEMS =
8 × 350 = 2800` (`kernel/src/desk/global.rs:31,76,79`) `Object`-Einträge plus
ein indexgleiches `ScreenInfo`-Array; bei LP64-`Object` ≈ 32 B sind das
≈ 90 KB + Parallelarray.

**Urteil:** gegen `TPA_SIZE = 4 MiB` (`arena.rs:52`) ist das unkritisch.
**[A]** Der Zuschlag für `alloc`/`core::fmt`/Formatierung, die im Kern heute
mit dem Rest geteilt werden und im `.NAP` eigene Kopien bräuchten, ist **nicht**
in den 228 KiB enthalten; auch mit einer Verdopplung bleibt reichlich Luft.
Die Zahl wird in U6 am `hello-gem` kalibriert und in U8 am `NEWDESK.NAP`
nachgemessen — als Messwert im Stufenbericht, nicht als Erwartung.

---

## 3. Was seit dem 2026-09-01 gebaut wurde

| Alte Stufe | Stand | Beleg |
|---|---|---|
| **S0** Spec `docs/api/aes/shell-loop.md` | **fertig**, 1109 Z. | gemessen |
| **S1a** `menu_bar(NULL, 0)` muss wirken | **fertig** — der Hide-Pfad liegt **vor** der Null-Prüfung und `remove_bar()` ist unbedingt | `tos-api/src/aes/menu.rs:338-362`, gegen `gemmnlib.c:726-727` |
| **S1b** AES-`DESKTOP`-Baum + `#Q` + `ap_private` | **fertig** — `DESKTOP_ROOT_OBSPEC = 4419`, `set_aes_background`, `appl_init.ap_private = desktop_root_obspec()` | `tos-api/src/aes/init.rs:61-65,98,218`, `appl.rs:303` (`gemsuper.c:95-96`) |
| **S1c** `Fsetdta`/`Fgetdta` auf `p_xdta` | **fertig**, mit benanntem `KERNEL_DTA`-Rückfall genau für den EL1-Desktop | `tos-api/src/bdos/fs/find.rs:52-66,86-105` (`bdos/fsmain.c:396,403-405`) |
| **S1d** XBIOS `Keytbl` (0x10) | **fertig**, Tabellen in der TPA-Low-Region, User-VA zurück | `tos-api/src/xbios/keytbl.rs` |
| **S3** Shell-Loop | **fertig** — `sh_main`, `sh_ldapp` (4 Arme), `sh_chdef`, `sh_chgrf`/`sh_toalpha`/`sh_tographic`, `sh_draw_clear`, `ratinit`, `wm_new`, `#Z` über `process_inf2`; der `DESKTOP_APP`-Arm ruft weiter `crate::desk::run()` — **exakt der benannte, befristete Shim, den der Plan vorgesehen hat** | `kernel/src/shell_loop.rs`, insb. `:430` |
| **S7** `ap_id` | **fertig** — `reported_pid()` liefert `AP_CHAIN_PID = 0` für jede Identität; D-R1-5 geschlossen | `tos-api/src/aes/appl.rs:200-224` (`gemsuper.c:94`, `geminit.c:875-876`) |
| — | `save_windows`/`restore_windows` ohne Produktivaufrufer | `grep 'save_windows()' kernel/src tos-api/src` findet nur die Definitionen |
| — | Nachbarschaft: **PORT05** (`2711799`, HW-Abnahme 2026-09-26) — Elter über `Pexec` suspendieren | `TASK/PORT05.md` |
| — | Nachbarschaft: **ACC10** eröffnet — `appl_exit` blockiert headless in `wait_for_accs` | `TASK/ACC10.md` |

**Offen aus dem alten Plan:** S2 (EL0-Substrat), S4 (Typen nach `tos-types`),
S5 (Umzug), S6 (Umschalten), S8 (Löschen) — plus G1, G4, G5 und STOP-3.

---

## 4. Die Bündelung in S6 war ein Fehler — zwei Achsen statt einer

Der alte Plan macht in **S6** drei Dinge gleichzeitig:
das Prozessmodell (der Desktop stirbt, `sh_main` startet ihn neu), den
Adressraum (EL1 → EL0) und die Werkzeugkette (Kernel-Crate → `.NAP`). Und er
legt S6 ans Ende, unmittelbar vor die Abnahme.

**Das ist die riskanteste mögliche Reihenfolge, und sie ist unnötig.** Die
beiden Achsen sind unabhängig:

**Achse A — Prozessmodell.** Der Desktop bekommt einen echten Epilog, gibt
`done` zurück, terminiert, und `sh_main` startet ihn von oben neu; Programme
startet er nur noch über `shel_write`. Damit fallen `run_gem_app`,
`desktop_relinquish`, `desktop_resume`, `win_free_all` und die sechs
`shell_loop::sh_gem_*`/`sh_alpha_*`-Ausleihen weg — **das ist überwiegend
Löschen**, denn `sh_main` macht `wm_init(); ratinit(); sh_draw_clear()`
(`shell_loop.rs:96-99`), `sh_show` (`:467`) und `wm_new()` (`:503`) bereits
selbst; sie werden heute nur umgangen.
**Achse A braucht kein EL0.** Ihr Mechanismus ist durch A106 erwiesen (§2.4).

**Achse B — Verlagerung.** Neues Rust-`no_std`-EL0-Substrat, Umzug der 19
Dateien, Umschalten des `DESKTOP_APP`-Arms, Löschen von `kernel/src/desk/`.
Nach Achse A ist das eine **Verlagerung ohne Semantikänderung** — und genau das
ist die Art Änderung, die ein abdriftgefährdetes Projekt überlebt, weil jede
Abweichung im Diff sichtbar ist statt im Verhalten.

**Zwei weitere Gründe, A vor B zu ziehen:**

* Achse A ist der Teil, der die **belegte Konformitätslücke** schließt (G1). Sie
  entfernt einen Eigenbau, der bisher vier Tickets erzeugt hat — DESK35,
  DESK37, A96, A28/A29 — und der `desktop_relinquish` heißt.
* Achse B kann **datumsbedingt scheitern** (§7). Achse A kann einzeln
  abgenommen werden und hinterlässt einen Desktop, der EmuTOS' Struktur hat.

---

## 5. Der Schnitt — zehn Einheiten

Jede Einheit endet mit **einem laufenden Desktop**. Jede nennt: was sie ändert,
was sie beweist, und was wahr sein müsste, um danach aufzuhören und
auszuliefern.

Verbindlich für jede Einheit: **der Quittungsbericht trägt (a) die Zahl aus der
Driftprobe, (b) die Liste der entfernten Umgehungen mit dem Ersatz, der die
Entfernung kostenlos gemacht hat, (c) jede berührte `fn` ohne EmuTOS-Zitat.**
Begründung in §6.

---

### U1 — Erst das Instrument: den Desktop sichtbar machen

**Was sich ändert.** Kein Produktverhalten.
1. `kernel/src/arch/aarch64/exceptions.rs:344,483` — das Tor `dbg_from_el0 =
   (frame.spsr & 0xF) == 0` schließt den Desktop aus dem AES-/GEMDOS-Tracer aus.
   Seit DESK10 geht der Desktop **wirklich** über `svc #2`/`svc #1`
   (`kernel/src/desk/mod.rs:150`, `dosbind.rs:127`) — er wird nur deshalb nicht
   aufgezeichnet, weil sein `svc` aus EL1 kommt. Das Tor auf „jeder `svc`"
   erweitern (der Tracer ist per `QED_SVC_TRACE = false` ohnehin ausgebaut).
2. `kernel/src/dbg_qedtrace.rs:14-17` sagt *„The desktop runs at EL1 and calls
   the AES/VDI directly (never through the `svc` path)"*. **Das ist seit DESK10
   falsch** und muss korrigiert werden — nach
   [[justification-is-copied-to-several-places]] ist die Kopie in
   `exceptions.rs:341-345` mitzusuchen.
3. `tools/desk-drift-probe.sh` einchecken: die Zählung aus §2.2 als ein
   reproduzierbarer Befehl, der eine Zahl ausgibt.

**Was es beweist.** Ein Trace eines Desktop-Boots, gegen
`desk/deskmain.c:1950-2073` gestellt: `appl_init → graf_handle →
wind_get(DESKWH,WF_WXYWH) → … → menu_bar(tree,1)`. Ab dieser Einheit ist die
Aufrufsequenz des Desktops **diffbar gegen die Quelle** — mit demselben
Instrument, das D-R1-5 und D-R1-6 gefunden hat.

**Aufhören und ausliefern?** Jederzeit. Das Produkt ist unverändert.

---

### U2 — Der Zustandskanal: `app_start`/`app_save` über `shel_get`/`shel_put`

**Was sich ändert.** Zwei AES-Bindungen in `kernel/src/desk/mod.rs` (Opcodes
122/123, serverseitig vorhanden: `tos-api/src/aes/mod.rs:837-838`), und
`inf.rs`' `app_start`/`app_save` bekommen EmuTOS' Reihenfolge:
`app_start` liest **zuerst** `shel_get(buf, SIZE_SHELBUF)` und nur bei
ungültiger Signatur die Datei (`deskapp.c:832-836`); `app_save(FALSE)`
serialisiert in den Puffer statt auf Platte (`deskapp.c:1293`), `app_save(TRUE)`
schreibt zusätzlich die Datei. Das 128-Byte-`CPDATA`-Präfix
(`deskapp.c:136,1123-1124`) wird respektiert. `SHELLBUF = 4192`
(`tos-api/src/aes/shell.rs`) stimmt mit `SIZE_SHELBUF` überein.

**Warum vor U3.** Weil ohne diesen Kanal ein sterbender Desktop seinen Zustand
verliert — und weil genau „der Desktop muss weiterfunktionieren" das ist, was
die Umgehungen zweimal am Leben erhalten hat (§6). Der Ersatz kommt zuerst,
dann die Entfernung.

**Was es beweist.**
1. Headless raspi4b: Desktop hoch, Fenster öffnen, **A106-Restart** auslösen
   (`shel_write(SHW_RESCHNG)`) — der neue Desktop kommt mit **denselben**
   Fenstern aus dem Shell-Puffer, nicht aus der Datei. Screendump gegen den
   Vorzustand.
2. Kaltstart ohne `NEWDESK.INF`: unverändert Standardumgebung
   (`deskapp.c:833-834`).
3. *Save desktop* schreibt weiter korrekt; `fsck_msdos` sauber.
4. Beide Boards bauen; Boot-Screendump ohne Regression.

**Vorhersage.** Der erste Bruch ist das `CPDATA`-Präfix — ein Puffer, dessen
erste 128 Byte als INF-Text gelesen werden, scheitert an der `#`-Signaturprüfung
und fällt still auf die Datei zurück. Zweiter Kandidat: `cnx_put()` schreibt die
`#W`-Zeilen heute in `G.g_cnxsave`; sie müssen in den serialisierten Text.

**Aufhören und ausliefern?** Ja, wenn 1–4 grün sind. Der Desktop stirbt danach
noch nicht; der Kanal liegt ungenutzt daneben und wird von A106 mitgetestet.

---

### U3 — Der Desktop wird ein Prozess, der endet (G1)

**Was sich ändert — überwiegend Löschungen.**
* `pro::pro_run` (`kernel/src/desk/pro.rs:121`, existiert) wird der **einzige**
  Startweg. `do_open`/Doppelklick routen über `pro_run(isgraf, cmd, tail)` →
  `shel_write(SHW_EXEC, …)` → `done = TRUE`, wie `deskpro.c:52-64` und
  `desksupp.c:951`. Die beiden `crate::launch::run_program_with_tail`-Aufrufe
  (`main.rs:1022,1090`) fallen weg.
* **Gelöscht:** `run_gem_app` (`main.rs:1088-1094`), `desktop_relinquish`
  (`:1107-1160`), `desktop_resume` (`:1166-1230`), `win_free_all`
  (`:1234-1239`), `launch_native`s App-/Console-Arme
  (`:988-1046`), sowie in `kernel/src/shell_loop.rs` die nur dafür
  existierenden `sh_gem_enter`, `sh_gem_show`, `sh_gem_leave`,
  `sh_alpha_enter`, `sh_alpha_leave`, `sh_run_pending_console` (`:274-398,571-587`).
  `sh_main` zieht diese Schritte selbst (`:94-99,467,503`).
* `desk::run()` bekommt EmuTOS' Epilog in EmuTOS' Reihenfolge:
  `cnx_put(); app_save(FALSE); menu_bar(NULL,0); wind_set(DESKWH,WF_NEWDESK,NULL,0,0);
  appl_exit();` (`deskmain.c:2154-2166`), dann Rückkehr.
* `sh_ldapp`s DESKTOP-Arm (`shell_loop.rs:418-437`) ruft vor `desk::run()`
  `desk::reset()` — der von A106 belegte Abbau (§2.4).
* `do_read_inf`s benannte Abweichung (`main.rs:1855-1871`,
  „**no desktop restart**", DESK14 §4) wird eingelöst:
  `restart_desktop = TRUE; done = TRUE` (`deskmain.c:919-925`).
* `ap_private`/`ap_id` werden **nicht** angefasst — sie stimmen bereits (§3).

**Was es beweist.**
1. **Reale Pi 4** (interaktiv, [[desktop-interactive-verification-real-hw]]):
   Doppelklick auf `QED.NAP` → QED im Vollbild → beenden → der Desktop steht
   mit **denselben** Fenstern wie vorher, rekonstruiert aus
   `shel_get`/`cnx_get`.
2. *File → Execute EmuCON* läuft über den `CONSOLE_APP`-Arm von `sh_main`
   (`gemshlib.c:614-622`) statt über `sh_run_pending_console`; Screendump der
   Konsole und des Desktops danach.
3. Headless raspi4b: `#Z 01 C:\HELLO2.NCP@` unverändert; A106-Restart weiter
   0 px.
4. Driftprobe: die `shell_loop`- und `launch`-Gruppen aus §2.2 D sind **weg**.
5. AES-Trace (U1) des Doppelklicks: `shel_write(121)` ist die letzte
   Desktop-Aktion vor `appl_exit(19)`; kein `Pexec` aus dem Desktop.

**Named blocker.** **ACC10** (`TASK/ACC10.md`): `appl_exit` → `wait_for_accs
(AP_ACCLOSE)` schläft auf `WAIT_INPUT` (`tos-api/src/aes/acc.rs:164`), das
headless nur Taste oder Zeigerbewegung weckt. Nach U3 durchläuft der Desktop
`appl_exit` **bei jedem Programmstart** statt einmal. Ohne geladenes Accessory
ist `all_have()` über eine leere Menge sofort wahr (`acc.rs:124-131`) und die
Schranke fällt durch — **mit** Accessory hängt der headless Nachweis.
**ACC10 gehört vor U3**, sonst ist Punkt 3 nicht fahrbar.

**Aufhören und ausliefern?** **Ja — und das ist die wichtigste Stelle des
Plans.** Nach U3 hat der Desktop EmuTOS' Prozessstruktur: er terminiert, teilt
`rlr` mit der gestarteten Anwendung, startet nichts selbst. Er liegt nur
weiterhin auf EL1. Für die Vorführung ist das ein **besserer** Zustand als
heute, weil der Eigenbau weg ist, der DESK35/DESK37/A96 erzeugt hat.
Bedingung: 1–5 grün, davon 1 und 2 auf echter Hardware.

---

### U4 — Der Kernel braucht `desk::` nicht mehr

**Was sich ändert.**
* Eigener Endungstest für `\AUTO\` und `C:\ACC\` im Kernel statt
  `crate::desk::ftype::classify` (`launch.rs:266`, `accs.rs:48`) — 1:1 zu
  `bios/bios.c:713-714`.
* `check_alt_letter_key` (`main.rs:1472-1487`) liest `Keytbl(-1,-1,-1)->shift[]`
  statt `crate::keymap::to_ascii` — genau `deskmain.c:1144-1145`. Der Trap
  existiert (`tos-api/src/xbios/keytbl.rs`).
* **[STOP-3] Entscheidung `console_pause_for_key`** (`main.rs:1243-1249`).
  Empfehlung wie im alten Plan: **ersatzlos streichen.** EmuTOS hat es nicht;
  `sh_chgrf` schaltet direkt zurück (`gemshlib.c:513-523`). Die
  Doc-Begründung im Code beruft sich auf „classic TOS", nicht auf EmuTOS, und
  unser Profil ist EmuTOS Falcon030/TOS4
  ([[emutos-target-profile-falcon-tos4]]). Will der Anwender es behalten, ist es
  eine **benannte, in `docs/api/` einzutragende Abweichung** — nicht eine
  stillschweigende. Damit fallen `crate::fbcon::write_byte` und
  `tos_api::host::host()` weg.
* Die vier `crate::initinfo`-Strings (`inf.rs:2339-2358`): `VERSION_STRING` und
  `AUTHORSHIP_LINE`/`INDEPENDENCE_NOTICE` werden Konstanten des Desktops,
  `machine_name()` kommt aus dem `_MCH`-Cookie, den der Loader ohnehin je
  Prozess in die TPA legt. **Hinweis:** `kernel/src/initinfo.rs:78` verweist auf
  `crate::desk::inf::do_aboutdesk` als gemeinsame Quelle — die Begründung liegt
  also **zweimal** und muss beidseitig geändert werden.
* `crate::kprintln` → `Cconws`; die vier unverdrahteten Selftests (`dir.rs:2557`,
  `inf.rs:4182,4289`, `ins.rs:1130`) bekommen eine Startstelle **oder** ein
  Ticket. [[unwired-selftests-rot-silently]] — sie beim Umzug still zu verlieren
  ist der Normalfall, wenn man nichts tut.

**Was es beweist.** `/usr/bin/grep -rn 'crate::desk' kernel/src --exclude-dir=desk`
liefert nur noch `pub mod desk;` (`lib.rs:62`), den `DESKTOP_APP`-Arm
(`shell_loop.rs:430`) und `desk::reset()` (`lib.rs:475`). Boot-Screendumps beider
Boards unverändert; `\AUTO\`-Lauf und `.NAC`-Erkennung unverändert (Serial).

**Aufhören und ausliefern?** Ja. Danach ist der Umzug ein Verschieben von
Dateien ohne Kernel-Änderung.

> **── Trennlinie. Alles darüber ist ohne EL0 und ohne neue Werkzeugkette. ──**

---

### U5 — `Object`/`IconBlk`/`TedInfo` nach `tos-types`

**Was sich ändert.** Die drei POD-Typen plus die `G_*`/`OB_*`-Konstanten ziehen
von `tos-api/src/aes/object.rs` nach `tos-types`; `tos-api` re-exportiert, damit
**kein** heutiger Aufrufer sich ändert. `ObjTree` (der ABI-abhängige Accessor)
bleibt serverseitig — [[aes-object-abi-is-tree-abi]].
`tos-types` hat keine Abhängigkeiten (`tos-types/Cargo.toml`), ist also von
einem `programs/`-Workspace per Pfad benutzbar.

**Was es beweist.** `const _: () = assert!(size_of::<Object>() == 32)` für LP64
in `tos-types`; beide Boards bauen; `cargo xtask test` grün; Boot-Screendump
identisch.

**Vorhersage.** Reine Verschiebung. Wenn doch ein Delta auftaucht, liegt es an
einem beim Verschieben verlorenen `#[repr]` — die Layout-Assertions fangen genau
das.

**Aufhören und ausliefern?** Ja, trivial.

---

### U6 — Das EL0-Substrat für Rust (`programs/`, `newtos-gem`, `hello-gem`)

**Was gebaut wird.**
* `programs/` als eigener Cargo-Workspace. `Cargo.toml:11-13` schließt ihn
  bereits aus. **Der Grund ist Build-Konfiguration, nicht Abdriftschutz** —
  eigenes Linkerskript (nach `sdk/newtos.ld`), eigene `-C
  relocation-model=static`/`code-model=small`, eigenes `target/`, damit die
  Board-Footgun ([[cargo-xtask-board-target-footgun]]) nicht zuschlägt. Der
  Abdriftschutz kommt aus den `[dependencies]` des Crates, nicht aus dem
  Workspace.
* `programs/newtos-gem` — das SDK: `gem_trap`/`Aespb` und die aesbind-Wrapper
  (Umzug aus `kernel/src/desk/mod.rs`), die `dos_*`-Wrapper (Umzug aus
  `dosbind.rs`), die VDI-Wrapper (Umzug aus `vdibind.rs`), ein `GlobalAlloc`
  über den TPA-Heap `[_end, p_hitpa)`, `_start` (= `deskstart.S`:
  `Mshrink`-Äquivalent, `main`, `Pterm`), `panic_handler`.
  Entspricht EmuTOS `desk/gembind.c` + `include/{gemdos,bdosbind,vdibind}.h`.
* `programs/hello-gem` — minimaler `.NAP`: `appl_init` → `graf_handle` →
  `wind_get(DESKWH,WF_WXYWH)` → `wind_create/open` → `objc_draw` →
  `evnt_multi` → `wind_close/delete` → `appl_exit` → `Pterm`.
* `cargo xtask build-programs` (`tools/xtask/src/main.rs:41-46`),
  `kernel/build.rs` mit `include_bytes!` und **hartem Fehler**, wenn der Blob
  fehlt. Ein leerer Blob wäre ein Stub.

**Warum eingebettet und nicht als Datei auf `C:`** — unverändert wie im alten
Plan: EmuTOS' `aes_run_rom_program(deskstart)` (`gemshlib.c:570-580,606`) ist
genau das, kein Boot-Risiko bei fehlender Karte, und der magische Name
`DEF_DESKTOP` ist schon `b"NEWDESK"` (`tos-api/src/aes/shell.rs:80`).
**Neu, und es stärkt den Punkt:** die Ersetzbarkeit ist **bereits belegt** —
PORT05 hat mit `#Z 01 C:\PEXPAR.NAP@` eine EL0-Anwendung **statt** des
eingebauten Desktops gefahren (`measurement-2026-09-25-port05/method/newdesk-pex.inf`).
Der AUTORUN-Arm kann es schon; nur der DESKTOP-Arm ist noch festverdrahtet.

**Was es beweist.**
1. `readelf -h`: `ET_EXEC`, `EM_AARCH64`, `e_entry >= 0x1000`, kein
   `PT_DYNAMIC` — die Loader-Bedingungen (`tos-api/src/bdos/process/elf.rs:62-65`,
   `sdk/newtos.ld` Kopf).
2. `registry::register(b"HELLOGEM", &BLOB)` + `Pexec(PE_LOADGO, "HELLOGEM")`
   (`pexec.rs:218` löst die Registry **vor** FAT16 auf); QMP-Screendump zeigt
   das Fenster; das Programm terminiert und der Kernel läuft weiter.
3. **Gemessen und berichtet:** Imagegröße und Heapbedarf gegen
   `TPA_SIZE = 4 MiB` — die Kalibrierung für U8.
4. Beide Boards bauen; keine neuen Clippy-Warnungen.

**Vorhersage.** Der erste Fehlschlag ist nicht das ELF-Laden (zwei Selftests
fahren den Pfad: `selftest_pexec.rs:238`, `selftest_pexec_d4d5.rs:140`), sondern
(a) die Heapgrenze — `_end` … `p_hitpa` korrekt aus der Basepage ableiten, ohne
den Stack zu treffen (`sdk/newtos.ld` reserviert die oberen 64 KiB) — oder
(b) ein `PT_DYNAMIC`/GOT aus fehlenden Codegen-Flags.
**Kein** ABI-Risiko: ein LP64-EL0-Prozess liefert dasselbe `Abi::Lp64` wie der
EL1-Desktop heute ([[native-64bit-abi-future]]).

**Aufhören und ausliefern?** Ja — nichts im Bootpfad geändert außer einem
auskommentierbaren Hook.

---

### U7 — Die letzten zwei VDI-Konstanten (G4, G5)

**Was sich ändert.**
* `CELL_W`/`CELL_H` → `graf_handle` (Opcode 77). Die Bindung existiert
  (`kernel/src/desk/mod.rs:501`); `gl_wchar`/`gl_hchar` sind dieselben 8/16
  (`tos-api/src/aes/graf.rs`, `tos-api/src/vdi/font.rs:19,21`). Zieht
  `icons.rs`' `IB_WTEXT`/`IB_HTEXT` von `const` auf `fn`. Acht Dateien,
  ≈ 200 Nutzungen, **null beobachtbarer Unterschied**.
* `SMALL_CELL_W`/`SMALL_CELL_H` → `vst_height(gl_ws.ws_chminh, …)` auf der
  **eigenen** Workstation, die `vdibind.rs` bereits öffnet — der Weg jeder
  GEM-Anwendung (`gemgraf.c:407`). Damit ist G5 zu, ohne AES-Erweiterung, und
  **[STOP-2]** des alten Plans (6×6-Font, Tracker C1/E3) entfällt als Blocker.

**Warum hier und nicht in Achse A.** ≈ 200 Stellen ohne beobachtbare Wirkung
sind 13 Tage vor der Vorführung ein schlechter Tausch. Sie gehören dorthin, wo
sie einen Zweck haben: das umgezogene Crate soll `tos_api::vdi::font` nicht mehr
kennen.

**Was es beweist.** Boot-Screendump beider Boards **pixelgleich** zum
Vorzustand — das ist bei Nulländerung die ganze Prüfung. Driftprobe: Gruppe B
aus §2.2 ist weg.

**Aufhören und ausliefern?** Ja.

---

### U8 — Die Verlagerung: `kernel/src/desk/` → `programs/desktop`

**Was passiert.** Die 19 Dateien ziehen um. `mod.rs`' aesbind, `dosbind.rs` und
`vdibind.rs` sind in U6 bereits nach `newtos-gem` gegangen. `SpinMutex` →
`RefCell`/`Cell` (`global.rs:18`, `inf.rs:34`). `kprintln` → `Cconws`. Die vier
Selftests werden ein EL0-Testprogramm (Vorbild `sdk/apps/apitest`).
**Semantik ändert sich nicht** — U2, U3, U4, U5, U7 haben sie schon
zurechtgerückt. **`NEWDESK.NAP` wird noch nicht gestartet.**

**Was es beweist.**
1. `cargo xtask build-programs` erzeugt `NEWDESK.NAP`; ELF-Bedingungen wie U6;
   **Image- und Heapgröße gemessen** gegen 4 MiB, gegen die U6-Kalibrierung.
2. **Driftprobe meldet 0**, und `programs/desktop/Cargo.toml` listet genau
   `newtos-gem` + `tos-types`.
3. Kernel + Boot unverändert, Screendumps identisch, beide Boards bauen.

**Vorhersage.** Teuerster Einzelposten ist nicht die Menge, sondern
`global.rs`' `SpinMutex` → `RefCell`: dort, wo `global::with(|g| …)` heute
verschachtelt gerufen wird, gibt es Borrow-Konflikte. `global::with` nimmt
bereits die Sperre und ist nicht wiedereintrittsfähig — die Stellen sind im
Baum schon kommentiert (`main.rs:1196-1199`).

**Aufhören und ausliefern?** Ja — zwei Desktops im Baum, nur der alte verdrahtet.
**Das ist das Risikofenster dieses Plans** und es endet erst mit U10 (§6).

---

### U9 — Der Umschalter

**Was sich ändert.** `sh_ldapp`s `DESKTOP_APP`-Arm (`shell_loop.rs:418-437`)
lädt `D.s_cmd` aus der Registry statt `crate::desk::run()` zu rufen. `s_cmd`
wird von `sh_chdef` aus `sh_cdir` + `sh_desk` gebaut
(`tos-api/src/aes/shell.rs:844-880`), `sh_desk` ist bereits `b"NEWDESK"`
(`:80`) — der Registry-Schlüssel muss zu dieser Schreibweise passen; das ist
die eine Detailentscheidung dieser Einheit. Der alte Pfad bleibt hinter dem
Cargo-Feature `desk-in-kernel`; Default **zunächst alt**, Umschaltung erst nach
grünem Testerlauf.

**Was es beweist.**
1. Boot bis Desktop, QMP-Screendump beider Boards — visuell identisch zum
   in-kernel-Desktop.
2. **Reale Pi 4:** Fenster öffnen/verschieben/schließen, Menü, Doppelklick auf
   `HELLO2.NCP` und `QED.NAP`, Rückkehr mit denselben Fenstern.
3. *Save desktop* schreibt `NEWDESK.INF`; `fsck_msdos` sauber.
4. AES-Trace (U1) des EL0-Desktops gegen den des EL1-Desktops aus U1: **dieselbe
   Aufrufsequenz**. Das ist der eigentliche Beweis, dass die Verlagerung
   semantikfrei war.
5. Feature-Flag zurück → alter Zustand.

**Vorhersage.** `desk::reset()`/`global::reset()` verschwinden hier ersatzlos —
der Prozess stirbt und nimmt seinen Adressraum mit. Wenn danach etwas fehlt,
liegt es im AES-seitigen Zustand, den `reinit()` bisher mit abgeräumt hat.
Zweite Vorhersage: der Mauszeiger steht nach dem ersten Desktop-Neustart
anders, weil `ratinit()` ihn setzt.

**Aufhören und ausliefern?** Ja, mit dem Flag auf alt.

---

### U10 — Löschen (Pflichtstufe)

**Was sich ändert.** `kernel/src/desk/` (19 133 Z.) weg, `pub mod desk;`
(`lib.rs:62`) weg, Feature `desk-in-kernel` weg, `KERNEL_DTA`
(`tos-api/src/bdos/fs/find.rs:52-66`) weg — die Doc dort nennt diesen Plan
bereits als Auslöser. `save_windows`/`restore_windows`
(`tos-api/src/aes/window.rs:1419,1444`) weg, mitsamt den zwei Tests, die ihre
Aufrufreihenfolge als Vertrag festschreiben
(`tos-api/tests/aes_app_return_descriptor.rs:332,341`,
`aes_launch_boundary_reset.rs:337,344`).

**Was es beweist.** `grep -rc 'crate::desk' kernel/src` = 0; Delta der
Kernel-Binärgröße im Bericht (beide Boards); voller Regressionslauf: Boot +
Selftests + Screendumps + `fsck_msdos`, beide Boards, interaktiv auf realer Pi 4.

**Aufhören und ausliefern?** Das ist das Ende. **Der Plan gilt nicht als
abgeschlossen, solange `kernel/src/desk/` existiert** — §6 erklärt, warum das
kein Aufräumen, sondern die Maßnahme ist.

---

### Abhängigkeiten

```
U1 ──────────────────────────────────────────────────────┐
ACC10 ──► U2 ──► U3 ──► U4 ─┬─► U5 ──┬─► U8 ──► U9 ──► U10
                            ├─► U6 ──┤
                            └─► U7 ──┘
```
Seriell, ein Task pro Agent ([[limit-concurrent-workers-token-budget]]).

---

## 6. Was den dritten Abdrift verhindert — die Quellenlage

### 6.1 Was die beiden Abdrifte waren, mit Beleg

Der Anwender hat die Randbedingung am 2026-09-01 gesetzt
(`docs/STATUS.md:540`). Die Quittungen benennen die beiden Abdrifte **nicht in
einer Liste**, aber jeder einzeln, mit Datum und Wirkung. Rekonstruiert:

**Abdrift 1 — `kernel/src/desktop.rs` (+ `desktop_icons.rs`), 2026-06-22
(`85051e3`) bis 2026-07-21 (`25deb42`).** ADR-016s Context
(`docs/MEMORY.md:584-604`) nennt Ursache und Wirkung: ein **handgeschriebenes**
Verzeichnisfenster-Subsystem (`DIRTREE`, `build_dir_*`, `draw_dir_listing`,
`redraw_*`, `VIEW_MODE`, `SORT_MODE`) statt EmuTOS' `FNODE`/`WNODE`/
`win_bldview`/`do_wredraw`. Wirkung: Icon-Beschriftungsüberlappung auf echter
Pi 4, View-Switch-Reste, ein Redraw nach Handle-Reihenfolge mit Vollbild-Clip.
Es war nie aus EmuTOS abgeleitet — es gab nichts, *wovon* es hätte abdriften
können, und genau das nennt ADR-016 „hand-rolled" und „untrustworthy".

**Abdrift 2 — `kernel/src/desk/` unter ADR-016, 2026-07-05 (`97789f0`) bis zur
Korrektur durch ADR-022, 2026-07-22.** ADR-022s Context
(`docs/MEMORY.md:1484-1498`) und der STATUS-Eintrag vom 2026-07-22
(`docs/STATUS.md:573`) benennen es sehr genau: eine **eigene**
`damage_redraw`-Kompositschleife, ein `full_redraw`-nach-jedem-Event-Shortcut,
Dialoge über direktes EL1-`objc_draw`, ein `DESK_PAINTER`-Hintergrundhook, und
ein fehlender `if (handle)`-Guard in WM_REDRAW gegen `deskmain.c:1275-1281`.
Wirkung: der Desktop trieb nie den app-zugewandten AES-Weg, sah dabei sauber aus
und **maskierte** die Fehler, an denen QED zerbrach.

**Und es gibt einen dritten, kleineren — schon innerhalb des laufenden
Versuchs**, und der ist als Beleg der nützlichste, weil er frisch ist:
2026-09-01 ließ sich im Verzeichnisbaum nicht scrollen, weil `desk/main.rs`
`WM_VSLID = 25`/`WM_HSLID = 26` **selbst deklariert** hatte — das Gegenteil von
`aesdefs.h:234-235`. Der anschließende maschinelle Abgleich fand 124 von 126
handdeklarierten AES-Konstanten korrekt und genau diese zwei falsch
(`docs/STATUS.md:540`). Zwei Wochen später fand der Rückbau vom 2026-09-14 neun
weitere Umgehungen, die **kein `grep` nach `tos_api::` sehen konnte**, weil sie
gar kein AES-Symbol berühren: handverkettetes `ob_add` statt `objc_add`,
direkte `ob_state`-Schreiber statt `menu_ienable`/`menu_icheck`, vier private
Kopien von `screen_dims()` statt `wind_get`
(`docs/analysis/desk-aes-gaps-2026-09-14.md` §2.1–2.3).

**Eine Lesart, die ich nicht ausschließen kann:** ob der Anwender am 2026-09-01
„`desktop.rs` + `desk/`" oder „`desk/` vor ADR-022 + `desk/` nach ADR-022"
gezählt hat, steht **nirgends**. Beide Lesarten führen zu demselben Mechanismus,
also ändert es nichts an der Antwort — aber gesagt sei, dass die Zählung selbst
nicht im Protokoll steht.

### 6.2 Die geprüfte Hypothese: „EL1 ist schneller, der direkte Aufruf ist bequem"

Geprüft, wie verlangt, gegen das Protokoll statt angenommen.

**Der Geschwindigkeitsteil ist nicht belegt — er ist nicht einmal erwähnt.**
`git log -S'faster' -- kernel/src/desk/` und `git log -S'overhead' --
kernel/src/desk/` liefern **null** Commits. Im ganzen heutigen `desk/` steht
das Wort „performance" **einmal**, in `win.rs:658`, und dort geht es um
EmuTOS' eigene Blit-Optimierung, nicht um den Trap. Es gibt im Protokoll
**keine einzige Stelle**, an der jemand eine Umgehung mit Laufzeit begründet
hätte. Wäre das der Grund gewesen, hätte er irgendwo gestanden — das Projekt
schreibt seine Begründungen auf, ausführlich.

**Der Bequemlichkeitsteil ist belegt, aber die Bequemlichkeit ist eine
andere, als vermutet.** Der Beleg steht in `aa6c39c` (2026-07-21,
„single-tasking screen handoff on GEM app launch"). Dieser Commit **erfindet**
`save_windows`/`restore_windows`/`clear_desktop_painter` in `tos-api` und lässt
den Desktop sie direkt rufen. Die Absicht im Text ist ausdrücklich
EmuTOS-treu — *„mirroring EmuTOS tearing its windows + clearing the screen when
a child runs"*. Das Verräterische ist nicht die Absicht, sondern die Form: die
drei Funktionen bekamen **keinen Opcode**. Der Rückbau hat genau das notiert:
*„the first two had no AES opcode at all, so no EL0 application could ever have
reached them"* (`343b3c5`). Die Bequemlichkeit war also nicht „ich spare mir den
Trap", sondern **„die strukturelle Antwort steht mir nicht zur Verfügung, die
lokale ist einen Bezeichner weit weg"** — die strukturelle Antwort (der Desktop
stirbt, `sh_main` macht `wm_init`/`sh_draw`/`wm_new`) setzte voraus, dass der
Desktop ein Prozess ist, und das war er nicht.

**Und es gibt eine Verschärfung, die der alte Plan nicht hatte.** Eine Umgehung
kann **mit einer schriftlichen Begründung** kommen, und eine plausible
Begründung überlebt Review beliebig lange. Der Modulkopf von `menu.rs` behauptete
seit `97789f0`:

> *„EmuTOS `men_update` calls `menu_ienable`/`menu_icheck`; in `tos-api` those
> aesbind calls **redraw the item unconditionally** … This is the desktop
> managing its own tree, not a workaround around the AES."*

**Das war falsch.** `menu_icheck` übergibt `dodraw = false`, `menu_ienable`
nimmt `dodraw` aus einem Bit, das `men_update` nie setzt — nachgelesen in
`aes/menu.rs:716-730` gegen `gemsuper.c:172,175-176`. Der Satz stand **vom
2026-07-05 bis zum 2026-09-14 im Baum: 71 Tage.** Entfernt hat ihn nicht ein
Review, sondern eine **Messung gegen die Quelle**.

### 6.3 Der Satz, auf den es ankommt

`343b3c5` (2026-09-15) sagt, warum die ersten beiden Reparaturversuche die
Umgehungen nicht losgeworden sind:

> *„The user asked for every function built into desk/ because the AES was not
> ready yet to be removed, explicitly allowing desk/ to break. **That permission
> is what makes this different from the two previous repair attempts: they had
> to keep working, so every bypass stayed.**"*

**Das ist der Mechanismus, und er steht in direkter Spannung zur Auflage dieses
Tickets** („jede Arbeitseinheit muss einzeln abnehmbar sein, nichts darf
hängen, der Termin ist der 9.–10. Oktober"). Die Anforderung „es muss
weiterlaufen" ist genau die, die zweimal jede Umgehung konserviert hat.

**Die Auflösung ist nicht, das Brechen zu erlauben** — der Termin verbietet es.
Sie ist eine Ordnungsregel:

> **Keine Einheit darf eine Umgehung stehen lassen, „weil es sonst nicht mehr
> läuft". Kann eine Einheit die Umgehung nicht kostenlos entfernen, steht sie
> an der falschen Stelle: erst den Ersatz bauen, dann entfernen.**

Genau dafür ist der Schnitt in §5 so geordnet: **U2 vor U3** (der
Zustandskanal, bevor der Desktop sterben darf), **U6 vor U8** (das Substrat,
bevor der Code umzieht), **U3 vor U9** (die Semantik, bevor der Adressraum).
Der Rückbau vom 2026-09-14 hat die Erlaubnis zu brechen einmal ausgegeben und
mit ihr die Lückenliste gekauft; die Ersatzteile — `sh_main`, `shel_write`,
`cnx_put`/`cnx_get`, `wm_new`, `form_dial` — sind seitdem gebaut. **Heute
kostet das Entfernen zum ersten Mal nichts.**

### 6.4 Die Maßnahmen, mit Datum ihrer Wirksamkeit

| | Maßnahme | Wirksam ab | Bewertung |
|---|---|---|---|
| 1 | **Der Bypass wird ein Compilerfehler.** `programs/desktop` hat weder `kernel` noch `tos-api` im Abhängigkeitsgraphen; die Symbole **existieren nicht** | **U10** | Die einzige Maßnahme, die den *Mechanismus* entfernt. Der alte Plan hat ihre Schwäche selbst benannt, und sie bleibt: von U8 bis U10 sind zwei Desktops im Baum |
| 2 | **Driftprobe als Zahl pro Einheit**, `tools/desk-drift-probe.sh`, im Quittungsbericht | **U1** | Muss monoton fallen. Eine Einheit ohne die Zahl ist nicht abgenommen. **Reicht allein nicht** — der Rückbau hat neun Umgehungen gefunden, die kein `grep` nach `tos_api::` sieht (§6.1) |
| 3 | **Funktionsnamen-Korrespondenz** (alter Plan §7.3) | **nie gebaut** | Ein Werkzeug, das nicht existiert, verhindert nichts. **Ersetzt durch:** jede in einer Einheit berührte `fn` **ohne** EmuTOS-`file:line` im Doc-Kommentar wird im Bericht aufgelistet. Der Baum zitiert heute fast durchgängig; die Ausnahmen sind die Driftkandidaten |
| 4 | **Der Desktop wird vom AES-Instrument beobachtet** | **U1, nicht U10** | Der alte Plan (§7.4) sagte, das komme mit dem Umzug. **Falsch:** der Desktop geht seit DESK10 über `svc #1`/`svc #2`; nur das Tor `dbg_from_el0` (`exceptions.rs:344`) schließt ihn aus. Eine Zeile, heute. Ab dann ist seine Bootsequenz gegen `deskmain.c:1950-2073` diffbar |
| 5 | **Akzeptanzkriterien sind TT-Messwerte** | bereits erfüllt | `ap_id == 0`, `ap_private == 0x1143` sind gebaut (§3). Für die verbleibenden Einheiten ist das Orakel der **Vergleich des AES-Traces vor/nach** (U9 Punkt 4) |
| 6 | **Die alte Implementierung wird gelöscht — eigene Stufe, eigenes Tor** | **U10** | Unverändert Pflicht. Dass zwei Desktops im Baum lagen, ist der Grund, warum Versuch 2 der Ausgangspunkt von Versuch 3 wurde |
| 7 | **Jede Einheit nennt Evidenz und Vorhersage, bevor sie läuft** | sofort | Eine falsch getroffene Vorhersage ist ein Befund |
| **8** | **Neu: keine Einheit lässt eine Umgehung stehen, „weil es sonst nicht läuft"** | **sofort** | §6.3. Die Regel, die aus `343b3c5` folgt und die der alte Plan nicht hatte |
| **9** | **Neu: eine geschriebene Begründung ist kein Beleg** | **sofort** | Der `menu.rs`-Kopf stand 71 Tage lang falsch im Baum. Jede Doc-Begründung, die eine Abweichung rechtfertigt, wird **gegen die genannte Quellzeile geprüft** oder gestrichen. [[no-rationalizing-emutos-deviations]], [[status-md-is-not-evidence]] |

**Ehrliche Einschränkung, unverändert:** Maßnahme 1 wird erst mit U10 wahr.
U8–U10 ist das Risikofenster. Was es gegenüber dem alten Plan verkleinert, ist,
dass in diesem Fenster **keine Semantik mehr geändert wird** — U2/U3/U4 haben
sie vorher zurechtgerückt, und U9 Punkt 4 prüft die Gleichheit mit dem
Trace-Diff statt mit dem Auge.

---

## 7. Der Termin — was vor dem 9. Oktober geht

Heute ist der 2026-09-26; die Vorführung ist am **9.–10. Oktober**
([[atari-meeting-demo-october]]), stehende Auflage: *nichts darf hängen,
sichtbare Fehler vor Konformität*.

**Empfehlung, datumsgebunden statt hoffnungsvoll:**

* **U1 sofort.** Kein Produktverhalten, und es ist das Instrument, mit dem alles
  Weitere geprüft wird.
* **ACC10, dann U2, dann U3** — *nur wenn sie bis etwa **3. Oktober** auf echter
  Pi 4 grün sind.* U3 ist der Teil, der die Vorführung besser macht: er entfernt
  den Eigenbau (`desktop_relinquish`/`desktop_resume`), aus dem DESK35, DESK37,
  A96 und ein Teil der PORT05-Nachbarschaft entstanden sind, und ersetzt ihn
  durch den Weg, den `sh_main` schon geht.
* **U4 ist nicht vorführungskritisch.** Wenn die Zeit knapp wird, entfällt es.
* **U5 bis U10 nach dem Termin.** Ein neues Rust-EL0-Substrat plus der Umzug von
  10 443 Codezeilen plus das Umschalten des Bootpfads ist **nicht** in 13 Tagen
  abnehmbar, und ein halb umgezogener Desktop ist der schlechteste aller
  Zustände.
* **Harte Grenze:** Ist U3 am **3. Oktober** nicht auf Hardware grün, wird der
  Stand eingefroren und erst am **11. Oktober** weitergemacht. Ein Rückbau am
  8. Oktober ist teurer als ein Verzicht am 3.

---

## 8. Was der Umzug kostet und was er bringt

Die Frage des Anwenders 2026-09-26: *was kostet es eigentlich, den Desktop auf
EL1 laufen zu lassen — ist das eine Sicherheitsfrage?*

### 8.1 Sicherheit ist der schwächste Grund, und das gehört gesagt

**Es gibt kein Bedrohungsmodell, das den Umzug rechtfertigt.** Einzelplatzrechner,
ein Benutzer, kein Netz gebaut (`TASK/NET01.md` … `NET06.md`, alle offen), und
das Original hatte **überhaupt** keinen Speicherschutz. Wer den Umzug mit
Sicherheit begründet, begründet ihn falsch. Er wird hier nicht als Grund geführt.

### 8.2 Die tragenden Gründe, mit Beleg statt Behauptung

**1. Der Desktop validiert den Anwendungspfad nicht — und das ist keine Theorie
mehr.** [[desktop-bypasses-app-abi-path]] sagt es seit Längerem; **PORT05 ist
der Vorfall**. Der Weg „EL0-Anwendung startet EL0-Anwendung" war kaputt,
vermutlich seit es ihn gibt, und niemandem aufgefallen, weil das eine Ding, das
ununterbrochen Anwendungen startet, ihn nicht benutzt. `TASK/PORT05.md` hält
den Trennversuch fest: *„Aus NewDesk funktioniert QED einwandfrei"* — der
Desktop war entlastet und damit zugleich als Prüfinstrument entwertet. Der
Defekt (`2711799`) war ein Elter, der vom eigenen Kind neu gestartet wurde, bis
die Prozesstabelle voll war und der vierzehnte `Pexec` mit `-39 ENSMEM`
antwortete. Ein gut aussehender Desktop hat das nicht verhindert und konnte es
nicht — er ging den Weg nie.
**Nach U9 geht der Desktop denselben Weg wie QED und TeraDesk.** Bricht der
Weg, bricht der Desktop, und zwar beim ersten Boot statt beim ersten Anwender.

**2. Fehlereingrenzung — nicht Sicherheit. Und heute noch nicht geschenkt.**
Ein Defekt in `desk/` ist heute ein **Kernel**-Defekt: `panic!` landet in
`kernel/src/panic.rs` und endet in `halt()`. **[M] Ehrlich dazugesagt:** eine
Ausnahme aus EL0 endet heute **genauso** — `unhandled()` gibt `[EXC] …` aus und
ruft `halt()` (`kernel/src/arch/aarch64/exceptions.rs:1819,1865`). Der Umzug
liefert also **nicht** von selbst einen überlebenden Fehler; er liefert die
**Voraussetzung** dafür: ein Fehlschlag ist einem Prozess mit eigenem Adressraum
und eigenem Slot zurechenbar, den der Kernel freigeben kann
(`pterm::release_slot`). Aus `halt()` ein „Prozess beendet, Desktop neu
gestartet" zu machen, ist **eigene Arbeit und nicht Teil dieses Plans** — sie
wird aber überhaupt erst möglich. Für *„nichts darf hängen"* ist das
mittelfristig mehr wert als jede Zugriffsschranke; kurzfristig ist es noch
nichts.

**3. Ersetzbarkeit.** ADR-022s Endgame ist der 1:1-Desktop als **ersetzbare**
EL0-AES-Anwendung (TeraDesk, Thing). Kernel-Code kann man nicht ersetzen, nur
neu bauen. **[M] Der Mechanismus existiert bereits und ist gefahren:**
`#Z 01 C:\PEXPAR.NAP@` hat am 2026-09-25 eine EL0-Anwendung **statt** des
eingebauten Desktops gestartet
(`measurement-2026-09-25-port05/method/newdesk-pex.inf`, `shell_loop.rs:448`).
Nur der `DESKTOP_APP`-Arm ist noch festverdrahtet — U9 ist die letzte Zeile,
die fehlt.

**4. Mehrkern und Multitasking.** Ein Desktop, der kein Prozess ist, kann nicht
einer von mehreren sein. Das ist die Richtung aus
[[post-2a-roadmap-bios-freemint]] (kooperativer FreeMiNT-tauglicher Kern,
Mehrkern-Vorbereitung); ohne diesen Umzug beginnt sie mit einer Ausnahme.

### 8.3 Die Nuance, die verändert, was „Konformität" hier heißt

**EmuTOS' User-Modus schützt nichts.** Es ist ein Bit in einem flachen
Adressraum ohne MMU: `aes/gemstart.S:126` macht `Super()`, um in den User-Modus
*zurück*zugehen, `aes/gemasm.S:77-79` springt „in user mode" in das
TEXT-Segment eines Accessories — aber der EmuTOS-Desktop *könnte* in den Kernel
schreiben, er tut es bloß nicht.

Wir bilden EmuTOS' **Struktur** nach — der Desktop ist ein Prozess und teilt
`rlr` mit der gestarteten Anwendung (`gemshlib.c:604,634`) — mit einem
**stärkeren Mechanismus**, als das Original hatte. Daraus folgt zweierlei, und
beides gehört in die Bewertung:

* **Der Umzug kostet uns mehr als ein Bit.** Jede der direkten
  `tos_api::`-Verbindungen muss eine echte Adressraumgrenze überqueren, die es
  auf einem Atari nie gab. Deswegen ist `Keytbl` bei uns eine Tabelle in der
  TPA-Low-Region und bei EmuTOS ein ROM-Zeiger; deswegen brauchte `p_xdta`
  einen `KERNEL_DTA`-Rückfall; deswegen ist `gl_wschar` bei EmuTOS eine Globale
  und bei uns ein `v_opnvwk`.
* **„1:1 wie EmuTOS" ist an dieser Stelle strenger als EmuTOS.** Wo unser
  Maßstab „was eine EL0-Anwendung könnte" mit EmuTOS' Praxis auseinanderfällt —
  G5 ist der Fall —, ist das kein Konformitätsverstoß, sondern eine bewusst
  strengere Auslegung, und sie muss benannt stehen, nicht stillschweigend gelten
  (`docs/analysis/desk-aes-gaps-2026-09-14.md` §5 sagt es bereits).

---

## 9. Was ich nicht ermitteln konnte

1. **[?] Welche zwei Rebuilds der Anwender am 2026-09-01 gezählt hat.** §6.1.
   Beide Lesarten führen auf denselben Mechanismus; die Zählung selbst steht
   nirgends.
2. **[?] Ob Achse A ohne ACC10 headless nachweisbar ist.** `all_have()` über
   eine leere Accessory-Menge ist sofort wahr (`tos-api/src/aes/acc.rs:124-131`),
   also sollte ein Lauf **ohne** `.NAC` durchkommen — aber
   `TASK/ACC10.md` berichtet, dass ein **kernelgestartetes** Programm an
   derselben Stelle stehenbleibt, und die Kontroll-Logs
   (`measurement-2026-09-25-port05/serial/control-kernel-launch-*.log`) stützen
   das. Ich habe das nicht gefahren. **ACC10 steht deshalb als Vorbedingung von
   U3 im Graphen, nicht als Risiko.**
3. **[A] Ob `MAX_USER_BLOCKS = 32` (`tos-api/src/bdos/process/usermem.rs`)
   reicht.** Die Allocator-Empfehlung (TPA-Heap) umgeht die Grenze; ich habe die
   Zahl nicht gegen einen realen Lauf geprüft.
4. **[A] Ob ein *dauerhaft* in `evnt_multi` sitzender EL0-Client sich verhält
   wie der heutige EL1-Aufrufer.** QED ist EL0-Client, sitzt aber nicht dauerhaft
   darin. Erster Messpunkt: `hello-gem` in U6. Unverändert offen seit dem alten
   Plan §9/3.
5. **Ich habe in diesem Job keinen Build gefahren.** Der `target/`-Baum wird mit
   parallelen Aufgaben geteilt ([[cargo-xtask-board-target-footgun]]). Die
   Größenmessung in §2.5 liest ein **vorhandenes** Artefakt
   (`target/aarch64-unknown-none-softfloat/release/kernel`, 2026-09-26 01:20) und
   hat nichts erzeugt. „Der Baum baut sauber auf beiden Boards" ist damit
   **nicht** von mir gemessen.
6. **Freie ADR-Nummer.** ADR-023 ist vergeben (`docs/MEMORY.md:1583`). Der
   Desktop-ADR braucht die nächste freie; ich habe die Vergabe nicht geprüft.

---

## 10. Entwurf ADR-0xx (Nummer offen, **nicht** 023)

> **ADR-0xx: Der Desktop ist ein Prozess, der endet — erst strukturell, dann auf EL0**
> **Status:** Entwurf (Anwenderentscheidung ausstehend). Löst ADR-022s Endgame ein.
> Ersetzt den ADR-023-Entwurf in `docs/analysis/desktop-userspace-plan.md` §11
> (dessen Nummer belegt ist und dessen Stufenplan hier durch §5 abgelöst wird).
>
> **Entscheidung, Teil A (Prozessmodell, ohne EL0).** `desk::run()` bekommt
> EmuTOS' Epilog (`desk/deskmain.c:2154-2166`), gibt `done` zurück und
> terminiert; `sh_ldapp`s DESKTOP-Arm startet ihn von oben neu
> (`aes/gemshlib.c:600-612`). Der Desktop startet nichts mehr selbst: er ruft
> `shel_write` und geht (`desk/deskpro.c:36-63`). Sein Zustand überlebt seinen
> Tod im AES-Shell-Puffer (`app_save` → `shel_put`, `shel_get` → `app_start`,
> `desk/deskapp.c:832,1293`). `run_gem_app`/`desktop_relinquish`/
> `desktop_resume` und die `shell_loop::sh_gem_*`-Ausleihen entfallen; AES-Lücke
> **G1** ist damit geschlossen.
>
> **Entscheidung, Teil B (Verlagerung).** `kernel/src/desk/` wird ein
> eigenständiges natives LP64-`no_std`-Rust-Programm (`programs/desktop` →
> `NEWDESK.NAP`), das als ELF-Blob im Kernel-Image liegt und über die
> Programm-Registry als regulärer EL0-Prozess startet (EmuTOS
> `aes_run_rom_program`, `aes/gemshlib.c:570-580,606`). Der Desktop hat danach
> **keine** Privilegien, die QED nicht auch hat. `kernel/src/desk/` **wird
> gelöscht**; der Plan gilt bis dahin als offen.
>
> **Reihenfolge ist Teil der Entscheidung.** A **vor** B. B ist danach eine
> Verlagerung ohne Semantikänderung; die Gleichheit wird per AES-Trace-Diff
> belegt, nicht per Augenschein.
>
> **Begründung.** (a) G1 ist eine belegte Abweichung von EmuTOS
> (`aes/gemshlib.c:600-612,632-645`), keine Entwurfsfreiheit. (b) Der Desktop
> validiert den Anwendungspfad nicht — PORT05 ist der Vorfall, nicht die These.
> (c) Ersetzbarkeit: der `#Z`-Weg ist bereits gefahren, nur der DESKTOP-Arm ist
> festverdrahtet. (d) Es ist der dritte Anlauf; die beiden früheren Abdrifte
> sind belegt (ADR-016 Context, ADR-022 Context) und ihr Mechanismus steht in
> `343b3c5`: *„they had to keep working, so every bypass stayed"*. Die Antwort
> darauf ist die Ordnungsregel „Ersatz vor Entfernung" und, am Ende, der
> Abhängigkeitsschnitt, der den Bypass zum Compilerfehler macht.
> **Sicherheit ist ausdrücklich kein Grund** — Einzelplatz, kein Netz, und das
> Original hatte gar keinen Speicherschutz.
>
> **Konsequenzen.** `programs/` wird ein zweiter Cargo-Workspace mit einem
> SDK-Crate `newtos-gem` (Grund: Linkerskript und Codegen-Flags, nicht
> Abdriftschutz). `Object`/`IconBlk`/`TedInfo` ziehen nach `tos-types`.
> `KERNEL_DTA` (`tos-api/src/bdos/fs/find.rs:52`) und
> `save_windows`/`restore_windows` (`tos-api/src/aes/window.rs:1419,1444`)
> entfallen. `graf_handle` ersetzt `CELL_W`/`CELL_H`, `vst_height` auf der
> eigenen Workstation ersetzt `SMALL_CELL_W`/`SMALL_CELL_H`. **ACC10 ist
> Vorbedingung.** `console_pause_for_key` wird gestrichen oder als benannte
> Abweichung in `docs/api/` eingetragen.
>
> **Verweise.** ADR-022 (1:1-Rebuild durch den AES-Trap — dies löst dessen
> Endgame ein), ADR-021 (EL0-Speicherfenster), ADR-018 (`.NAP`), ADR-013
> (`.NRSC`), `docs/api/aes/shell-loop.md`,
> `docs/analysis/desk-aes-gaps-2026-09-14.md` (G1–G7),
> `docs/analysis/desktop-userspace-plan.md` (Vorgänger),
> `TASK/PORT05.md`, `TASK/ACC10.md`.
