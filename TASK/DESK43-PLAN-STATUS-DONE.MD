# DESK43 (Planteil) — STATUS: DONE (2026-09-26) — Schnitt liegt vor, Umsetzung nicht begonnen

**Auftrag:** `docs/analysis/desktop-userspace-plan.md` (2026-09-01) gegen den
heutigen Baum pruefen, den Ausgangspunkt neu messen, in einzeln abnehmbare
Arbeitseinheiten schneiden, und die wichtigste Randbedingung beantworten —
**was den dritten Abdrift verhindert**. Nachgereicht per Kurskorrektur des
Anwenders: was der Betrieb auf EL1 tatsaechlich kostet, ob das eine
Sicherheitsfrage ist, und die Hypothese „EL1 ist schneller, der direkte Aufruf
ist bequem" **gegen das Protokoll pruefen statt annehmen**.

Basis: Branch `desk-rebuild`, HEAD `0d5042b`. **Kein Produktcode geaendert.**
`kernel/`, `tos-api/`, `tos-types/`, `tools/`, `sdk/` sind unberuehrt;
`docs/STATUS.md` und `TASK/DESK43.md` sind unberuehrt (Anwender fuehrt beide).
Nichts committet, nichts geflasht, nichts ausgeworfen, keine Pi gefahren, kein
`git checkout/restore/clean/stash/reset` — nur lesende `git log`/`git show`/
`git ls-tree`. `/Volumes/BOOT` nicht angefasst. Kein Build gefahren (geteilter
`target/`-Baum, [[cargo-xtask-board-target-footgun]]); die Groessenmessung liest
ein **vorhandenes** Artefakt.

**Ergebnis:** [`docs/analysis/desktop-userspace-plan-v2.md`](../docs/analysis/desktop-userspace-plan-v2.md).
Der alte Plan bleibt liegen.

---

## 1. Was neu gemessen wurde

| Groesse | 2026-09-01 | **2026-09-26** | Methode |
|---|---|---|---|
| `kernel/src/desk/` | 9 234 Z. / 15 Dateien | **19 133 Z. / 19 Dateien** | `wc -l kernel/src/desk/*.rs` |
| davon Code (ohne Kommentar/Leer) | — | **10 443 Z.** | `grep -v '^\s*//'` |
| `tos_api::`-Treffer im Text | „ca. 40 Aufrufe" | **36**, davon **19 reine Doc-Kommentare** | `/usr/bin/grep -n 'tos_api::'` |
| echte `tos_api::`-Bindungen | 21 Symbole / 56 Call-Sites | **3 Symbolgruppen** (Objekttypen, VDI-Font-Konstanten, `host()` 1x) | §2.2 des Plans |
| `crate::`-Bindungen | „sechs" (davon 2 nur Doc) | **7 Symbolgruppen** | §2.2 D |
| Kernel haengt an `desk::` | 1 Stelle vermutet | **2** (`launch.rs:266`, `accs.rs:48`) | `grep -rn 'crate::desk'` |
| AES-Opcodes des Desktops | 22 | **33** | `desk/mod.rs:57-89` |
| unverdrahtete Selftests | 3 | **4** (`dir.rs:2557`, `inf.rs:4182,4289`, `ins.rs:1130`) | 0 Aufrufer |
| Commits seit dem Plan | — | **412**, davon **38** an `desk/` | `git log --since=2026-09-01` |

**Neu beantwortet — die groesste offene Frage des alten Plans.**
`kernel::desk::*` belegt im vorhandenen Release-Kern **233 928 Byte `.text`**
(von 697 160 gesamt), 8 190 B `.data`, 1 923 B `.rodata`; im Debug-Profil
245 060 B. `llvm-nm --print-size --demangle` auf
`target/aarch64-unknown-none-softfloat/release/kernel`. Gegen
`TPA_SIZE = 4 MiB` (`tos-api/src/bdos/process/arena.rs:52`) unkritisch. Der
Zuschlag fuer eigene `alloc`/`core::fmt`-Kopien im `.NAP` ist **[A]** und wird
in U6/U8 kalibriert.

---

## 2. Der Stand des alten Plans

**Gebaut seit dem 2026-09-01:** S0 (`docs/api/aes/shell-loop.md`, 1109 Z.),
S1a (`menu_bar(NULL,0)`, `aes/menu.rs:338-362`), S1b (AES-`DESKTOP`-Baum,
`ap_private = 0x1143`, `aes/init.rs:61-65,98`), S1c (`p_xdta`,
`bdos/fs/find.rs:52-105`), S1d (XBIOS `Keytbl`, `xbios/keytbl.rs`),
S3 (kompletter Shell-Loop, `kernel/src/shell_loop.rs`), S7 (`ap_id = 0`,
`aes/appl.rs:200-224`). **P6-005 aufgeloest** — `pending_launch()` existiert
nicht mehr. `save_windows`/`restore_windows` haben **keinen** Produktivaufrufer
mehr.

**Offen:** S2 (EL0-Substrat), S4 (Typen nach `tos-types`), S5 (Umzug),
S6 (Umschalten), S8 (Loeschen), plus AES-Luecken **G1**, **G4**, **G5** und
**[STOP-3]** (`console_pause_for_key`).

**Drei Stellen des alten Plans sind sachlich falsch, nicht nur veraltet:**
§3.7(1) („unser Desktop ist reiner als EmuTOS" — seit `vdibind.rs` unzutreffend,
und es war schon damals ein Lob fuer eine fehlende Faehigkeit), §5.3/17
(`display_at` „wandert in den Shell-Loop" — die Arbeit gab es nie, `vdi/mouse.rs
::ensure_init` zentriert wie EmuTOS' VDI), §11 (**ADR-023 ist vergeben**,
`docs/MEMORY.md:1583`).

---

## 3. Der Schnitt

Zehn Einheiten, jede endet mit laufendem Desktop. Der alte Plan buendelt in
Stufe S6 Prozessmodell **und** Adressraum **und** Werkzeugkette und legt das ans
Ende — das wird hier aufgeloest, weil das Prozessmodell **kein EL0 braucht** und
sein Mechanismus durch A106 belegt ist (zwei Neustarts, Kaltstart-Bild 0 px).

| # | Einheit | Kern |
|---|---|---|
| U1 | Instrument zuerst | Tracer-Tor `dbg_from_el0` (`exceptions.rs:344`) oeffnen, stale Doc in `dbg_qedtrace.rs:14-17` korrigieren, Driftprobe einchecken |
| U2 | Zustandskanal | `app_start`/`app_save` ueber `shel_get`/`shel_put` (122/123) |
| U3 | **G1**: der Desktop endet | Epilog + `pro_run`; `run_gem_app`/`relinquish`/`resume`/`sh_gem_*` geloescht |
| U4 | Kernel loest sich | eigener AUTO-/ACC-Endungstest, `Keytbl` statt `keymap`, STOP-3 entscheiden |
| — | **Trennlinie: alles darueber ohne EL0** | |
| U5 | Typen | `Object`/`IconBlk`/`TedInfo` nach `tos-types` |
| U6 | EL0-Substrat | `programs/`, `newtos-gem`, `hello-gem`, `build-programs`, TPA gemessen |
| U7 | **G4/G5** | `graf_handle`; `vst_height` auf der eigenen Workstation |
| U8 | Verlagerung | `desk/` → `programs/desktop`, nicht verdrahtet, Driftprobe 0 |
| U9 | Umschalter | `DESKTOP_APP`-Arm laedt `NEWDESK` aus der Registry |
| U10 | Loeschen | Pflichtstufe; der Plan ist vorher nicht fertig |

**Vorbedingung, die nicht im alten Plan stand:** **ACC10** gehoert **vor U3**.
Nach U3 durchlaeuft der Desktop `appl_exit` bei jedem Programmstart statt
einmal, und `wait_for_accs(AP_ACCLOSE)` schlaeft headless auf `WAIT_INPUT`
(`tos-api/src/aes/acc.rs:164`, `TASK/ACC10.md`).

**Termin.** U1 sofort; ACC10 → U2 → U3 **nur**, wenn bis ~3. Oktober auf echter
Pi 4 gruen. U5–U10 nach dem 9./10. Oktober — ein neues Rust-EL0-Substrat plus
10 443 Codezeilen Umzug plus Bootpfad-Umschaltung ist in 13 Tagen nicht
abnehmbar, und ein halb umgezogener Desktop ist der schlechteste Zustand.

---

## 4. Der dritte Abdrift — was das Protokoll sagt

**Die beiden Abdrifte sind belegt, jeder einzeln, mit Datum und Wirkung.**

1. `kernel/src/desktop.rs` (`85051e3` 2026-06-22 … `25deb42` 2026-07-21):
   handgeschriebenes Verzeichnisfenster-Subsystem statt `FNODE`/`WNODE`/
   `win_bldview`/`do_wredraw` → Icon-Label-Ueberlappung, View-Switch-Reste,
   Redraw nach Handle-Reihenfolge (ADR-016 Context, `docs/MEMORY.md:584-604`).
2. `kernel/src/desk/` unter ADR-016 (`97789f0` 2026-07-05 … ADR-022
   2026-07-22): eigene `damage_redraw`-Kompositschleife, `full_redraw`-Shortcut,
   Dialoge per direktem EL1-`objc_draw`, `DESK_PAINTER`-Hook, fehlender
   `if (handle)`-Guard → der Desktop trieb den app-zugewandten AES-Weg nie und
   **maskierte** die Fehler, an denen QED zerbrach (ADR-022 Context
   `docs/MEMORY.md:1484-1498`, `docs/STATUS.md:573`).

**Was das Protokoll nicht sagt:** welche zwei der Anwender am 2026-09-01
gezaehlt hat. `docs/STATUS.md:540` haelt die Entscheidung fest, nicht die
Zaehlung. Beide Lesarten fuehren auf denselben Mechanismus.

**Die Hypothese des Anwenders, geprueft:**

* **Geschwindigkeit: nicht belegt, nicht einmal erwaehnt.**
  `git log -S'faster' -- kernel/src/desk/` und `-S'overhead'` liefern **null**
  Commits. Im ganzen heutigen `desk/` steht „performance" **einmal**
  (`win.rs:658`, ueber EmuTOS' Blit). Es gibt keine Stelle, an der jemand eine
  Umgehung mit Laufzeit begruendet haette.
* **Bequemlichkeit: belegt, aber eine andere als vermutet.** `aa6c39c`
  (2026-07-21) **erfindet** `save_windows`/`restore_windows`/
  `clear_desktop_painter` in `tos-api`. Die Absicht im Text ist ausdruecklich
  EmuTOS-treu; das Verraeterische ist, dass die drei **keinen Opcode** bekamen —
  *„no EL0 application could ever have reached them"* (`343b3c5`). Die
  Bequemlichkeit war nicht „ich spare den Trap", sondern **„die strukturelle
  Antwort steht mir nicht zur Verfuegung, die lokale ist einen Bezeichner weit
  weg"**: der Desktop war kein Prozess, also konnte er nicht sterben, also
  musste der Bildschirmuebergang von Hand gebaut werden.
* **Verschaerfung, die der alte Plan nicht hatte:** eine Umgehung kann **mit
  einer geschriebenen Begruendung** kommen, und die ueberlebt Review beliebig
  lange. `menu.rs`' Modulkopf behauptete seit `97789f0`, `menu_icheck`/
  `menu_ienable` zeichneten unbedingt. **Beide tun es nicht**
  (`aes/menu.rs:716-730` gegen `gemsuper.c:172,175-176`). Der Satz stand
  **71 Tage** im Baum und wurde nicht von einem Review entfernt, sondern von
  einer Messung gegen die Quelle.

**Der Satz, auf den es ankommt** — `343b3c5` (2026-09-15):

> *„That permission is what makes this different from the two previous repair
> attempts: **they had to keep working, so every bypass stayed**."*

Das steht in direkter Spannung zur Auflage dieses Tickets („einzeln abnehmbar,
nichts darf haengen"). Die Aufloesung ist **nicht**, das Brechen zu erlauben —
der Termin verbietet es —, sondern eine Ordnungsregel:

> **Keine Einheit laesst eine Umgehung stehen, „weil es sonst nicht mehr
> laeuft". Kann sie die Umgehung nicht kostenlos entfernen, steht sie an der
> falschen Stelle: erst den Ersatz bauen, dann entfernen.**

Genau danach ist §5 geordnet: U2 vor U3, U6 vor U8, U3 vor U9. Die Ersatzteile
(`sh_main`, `shel_write`, `cnx_put`/`cnx_get`, `wm_new`, `form_dial`) sind seit
dem Rueckbau vom 2026-09-14 gebaut — **heute kostet das Entfernen zum ersten Mal
nichts.**

**Neu gegenueber dem alten Plan, sofort wirksam:**
* Der Desktop wird **ab U1** vom AES-Tracer gesehen, nicht erst nach dem Umzug.
  Der alte Plan (§7.4) sagte das Gegenteil; seit DESK10 geht der Desktop
  wirklich ueber `svc #1`/`svc #2` (`desk/mod.rs:150`, `dosbind.rs:127`) und nur
  das Tor `dbg_from_el0` (`exceptions.rs:344`) schliesst ihn aus. Eine Zeile.
* Der Korrespondenz-Report des alten Plans (§7.3) **existiert nicht** und
  verhindert daher nichts. Ersetzt durch: jede beruehrte `fn` **ohne**
  EmuTOS-`file:line` wird im Quittungsbericht aufgelistet.
* Jede Einheit traegt drei Pflichtangaben: die Zahl der Driftprobe, die
  entfernten Umgehungen mit dem Ersatz, der die Entfernung kostenlos machte, und
  die Liste der Funktionen ohne Quellzitat.

---

## 5. Was EL1 kostet (Kurskorrektur des Anwenders)

**Sicherheit ist der schwaechste Grund und wird nicht als Grund gefuehrt.**
Einzelplatz, ein Benutzer, kein Netz gebaut (`TASK/NET01.md`…`NET06.md` alle
offen), und das Original hatte **gar keinen** Speicherschutz.

**Die tragenden Gruende:**
1. **Der Desktop validiert den Anwendungspfad nicht** — PORT05 ist der Vorfall,
   nicht die These. Der Weg „EL0 startet EL0" war kaputt, bis zur
   Prozesstabellengrenze und `-39 ENSMEM`, und niemandem aufgefallen, weil das
   eine Ding, das staendig Anwendungen startet, ihn nicht benutzt.
2. **Fehlereingrenzung — und ehrlich dazugesagt: heute noch nicht geschenkt.**
   Eine Ausnahme aus EL0 endet aktuell **genauso** in `halt()`
   (`kernel/src/arch/aarch64/exceptions.rs:1819,1865`) wie ein Kernel-Panic. Der
   Umzug liefert die **Voraussetzung** (Zurechenbarkeit auf einen Prozess mit
   eigenem Adressraum und freigebbarem Slot), nicht das Ergebnis. Das Ergebnis
   ist eigene Arbeit und **nicht** Teil dieses Plans.
3. **Ersetzbarkeit** — und die ist **bereits gefahren**: `#Z 01 C:\PEXPAR.NAP@`
   hat am 2026-09-25 eine EL0-Anwendung **statt** des eingebauten Desktops
   gestartet (`measurement-2026-09-25-port05/method/newdesk-pex.inf`). Nur der
   `DESKTOP_APP`-Arm ist noch festverdrahtet.
4. **Mehrkern/Multitasking** — ein Desktop, der kein Prozess ist, kann nicht
   einer von mehreren sein.

**Die Nuance:** EmuTOS' User-Modus schuetzt nichts — ein Bit, flacher
Adressraum, keine MMU (`aes/gemstart.S:126`, `aes/gemasm.S:77-79`). Wir bilden
EmuTOS' **Struktur** nach (Desktop ist Prozess, teilt `rlr` mit der gestarteten
Anwendung, `aes/gemshlib.c:604,634`) mit einem **staerkeren** Mechanismus.
Daraus folgt beides: der Umzug kostet uns mehr als ein Bit (jede direkte
Verbindung muss eine echte Adressraumgrenze ueberqueren, die es nie gab — daher
`Keytbl` in der TPA-Low-Region, daher `KERNEL_DTA`, daher `v_opnvwk` statt
`gl_wschar`), und „1:1 wie EmuTOS" ist an dieser Stelle **strenger** als EmuTOS.
Wo das auseinanderfaellt (G5), steht es benannt.

---

## 6. Offen, ausdruecklich nicht geraten

1. Welche zwei Rebuilds der Anwender gezaehlt hat — steht nirgends.
2. Ob U3 **ohne** ACC10 headless nachweisbar ist. `all_have()` ueber eine leere
   Accessory-Menge ist sofort wahr (`aes/acc.rs:124-131`), aber ACC10 berichtet
   einen Stillstand auch bei kernelgestarteten Programmen. **Nicht gefahren** →
   ACC10 steht als Vorbedingung im Graphen, nicht als Risiko.
3. Ob `MAX_USER_BLOCKS = 32` reicht — nicht gegen einen realen Lauf geprueft.
4. Ob ein *dauerhaft* in `evnt_multi` sitzender EL0-Client sich wie der heutige
   EL1-Aufrufer verhaelt. Erster Messpunkt: `hello-gem` in U6.
5. Kein Build gefahren; „der Baum baut sauber auf beiden Boards" ist **nicht**
   von mir gemessen.
6. Freie ADR-Nummer nicht ermittelt (023 ist vergeben).

---

## 7. Aufraeumen

Keine temporaere Verdrahtung angelegt. Geschriebene Dateien: genau zwei —
`docs/analysis/desktop-userspace-plan-v2.md` und diese Quittung. Scratch lag in
`$CLAUDE_JOB_DIR`-Scratchpad (`nm.txt`, `nmd.txt`) und ist geloescht.
