# DESK43 / U3 — STATUS: DONE (2026-09-26) — der Desktop endet, `sh_ldapp` startet

**Auftrag:** Einheit 3 von 10 des Desktop-Umzugs nach EL0
(`docs/analysis/desktop-userspace-plan-v2.md` §5 U3, „Der Desktop wird ein
Prozess, der endet (G1)"). Ueberwiegend Loeschung: `pro::pro_run` wird der
einzige Startweg, der Eigenbau ringsherum faellt weg, `desk::run()` bekommt
EmuTOS' Epilog, und `do_read_inf` loest seine benannte Abweichung ein.

Basis: Branch `desk-rebuild`, HEAD `d79e71f` (U2). Die Aenderung liegt
**uncommitted** im gemeinsamen Baum (zehn Dateien). Nichts geflasht, nichts
ausgeworfen, keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`. `/Volumes/BOOT` nicht angefasst.
`docs/STATUS.md`, `TASK/DESK43.md` und der Plan sind unberuehrt. Die
SD-Abbilder lagen im Job-Scratch und sind geloescht.

Messung:
[`measurement-2026-09-26-desk43-u3/`](../measurement-2026-09-26-desk43-u3/).
EmuTOS-Referenz: `references/emutos`.

---

## 1. Was geaendert wurde

Zehn Dateien, **−753 / +489 Zeilen**. Jede neue oder umgebaute `fn` traegt eine
EmuTOS-`file:line` im Doc-Kommentar (Massnahme 3 aus §6.4) — die Liste steht in
§5.

### 1.1 `pro::pro_run` ist der einzige Startweg (`kernel/src/desk/main.rs`)

Neu: **`pro_run_native(path, name, flavor, tail) -> bool`** —
`do_aopen`s Startschwanz (`desksupp.c:955-986`):
`strcpy(ptail, tail)` in `G.g_work + 1`, dann
`return pro_run(isgraf, app_path, G.g_work, G.g_cwin, curr)`. Ein `.NAC`
wird vorher abgewiesen (die alte, benannte NewTOS-Abweichung, ACC02).
Neu dazu: **`isgraf(flavor)`** = `pa->a_flags & AF_ISCRYS` (`desksupp.c:876`),
abgeleitet wie EmuTOS es aus der eingebauten `EMUDESK.INF` ableitet
(`deskapp.c:181-188` + `app_parse`, `:311-322`).

Die drei Aufrufer gehen jetzt alle dort durch:
* **`open_item`** (= `do_open`, `desksupp.c:1190-1280`) — der Doppelklick und
  *File → Open*. Gibt jetzt `do_open`s `WORD` zurueck.
* **`open_document`** — `do_aopen`s `installed_datafile`-Arm
  (`desksupp.c:924-951`). Gibt den `shel_write`-Erfolg zurueck.
* **`do_aopen_drop`** — der Drop einer Datei auf eine Anwendung
  (`deskfun.c:967` → `desksupp.c:955-986`). Gab bisher fest `false`; gibt jetzt
  `pro_run`s Antwort zurueck, also EmuTOS' `exit_desktop`.

Damit sind die beiden `crate::launch::run_program_with_tail`-Aufrufe aus
`desk/` verschwunden.

### 1.2 `done` wird durchgereicht

* `hndl_button`s Doppelklick-Arm: `done = do_filemenu(OPENITEM)`
  (`deskmain.c:988`) statt eines verworfenen `do_open_selected()`.
* `do_open_selected` (= `case OPENITEM:`, `deskmain.c:666-668`) → `bool`.
* `do_optnmenu` → `bool` (EmuTOS `static WORD do_optnmenu(WORD item)`,
  `deskmain.c:836`), `hndl_menu` reicht es weiter (`:1031-1033`).
* Neu: `static RESTART_DESKTOP: AtomicBool` = EmuTOS' `static BOOL
  restart_desktop` (`deskmain.c:359`), mit `clear_restart_desktop()` (`:1946`)
  und `restart_desktop()` (`:2136`).

### 1.3 Geloescht

| weg | war | Ersatz |
|---|---|---|
| `main::launch_native` | die Flavour-Weiche mit App-/Console-Arm | `pro_run_native` + `isgraf` |
| `main::run_gem_app` | relinquish → Loader → resume | `shel_write` + `sh_ldapp` |
| `main::desktop_relinquish` | Epilog nachgebaut, mitten im Lauf | `deskmain`s echter Epilog |
| `main::desktop_resume` | Prolog nachgebaut | `deskmain`s echter Prolog beim Neustart |
| `main::win_free_all` | ad-hoc WNODE-Freigabe | `desk::reset()` (Normalfall) / `close_desktop_windows` (Neustart) |
| `main::console_pause_for_key` | „Taste druecken" nach `.NCP` | — (EmuTOS hat es nicht; siehe §6) |
| `main::alert_launch_failed` | eigener Startfehler-Alert | `sh_main`s `fm_error` (`gemshlib.c:722-723`) |
| `main::build_cmd_tail` | ganzes Tail-Abbild bauen | `pro_run` baut Laengenbyte + CR selbst (`deskpro.c:56-57`) |
| `shell_loop::sh_gem_enter/sh_gem_show/sh_gem_leave` | `sh_main`-Schritte, aus `desk/` gezogen | `sh_main`/`sh_ldapp` ziehen sie selbst (`:715-720,632,676-677`) |
| `shell_loop::sh_alpha_enter/sh_alpha_leave` | Bildschirmwechsel, aus `desk/` gezogen | `sh_chgrf` (`:713`) |
| `shell_loop::sh_run_pending_console` | geschachtelter EmuCON-Lauf | `sh_ldapp`s `CONSOLE_APP`-Arm (`:614-622`) |

### 1.4 `desk::run()` ist `_deskstart`, `desk::deskmain()` ist `deskmain`

`kernel/src/desk/mod.rs`:

* **`run()`** = `deskstart.S:52-56` — `while !deskmain() {}`, also EmuTOS'
  `restart:`-Schleife; die Rueckkehr aus `run()` ist dort der `Pterm`.
* **`deskmain() -> bool`** = `deskmain.c:1938-2172`: `restart_desktop = FALSE`
  (`:1946`), der unveraenderte Prolog, `main::event_loop()` (`:2077-2123`),
  dann der Epilog **in EmuTOS' Reihenfolge**:
  `if (restart_desktop) { desk_busy_on(); close_desktop_windows(); desk_busy_off(); rc=FALSE; }
   else { cnx_put(); app_save(FALSE); rc=TRUE; }`,
  `menu_bar(NULL,0)` (`:2160`), `wind_set(DESKWH,WF_NEWDESK,NULL)` (`:2163`),
  `appl_exit()` (`:2166`), `if (rc==FALSE) wind_new();` (`:2168-2169`).
* **`close_desktop_windows()`** neu = `deskmain.c:1921-1930`.
* **`wind_new()`** neu in der aesbind-Flaeche (Opcode 109) — der Epilog braucht
  ihn; unser `win_free` loescht die AES-Handles nicht, genau wie EmuTOS' es
  nicht tut.

**Eine bewusste Umstellung gegenueber der C-Reihenfolge, benannt statt
stillschweigend:** EmuTOS gibt im Neustart-Arm den Speicher **vor**
`menu_bar(NULL,0)`/`wind_set(WF_NEWDESK,NULL)` frei (`:2140-2147`). `dos_free`
laesst die Bytes lesbar; ein Rust-`drop` nicht. Zwischen `:2141` und `:2163`
haelt die AES `g_screen` noch als Hintergrundbaum, und `menu_bar` kann sie ihn
lesen lassen. Deshalb macht `close_desktop_windows()` den **beobachtbaren**
Teil an EmuTOS' Stelle, und das Freigeben (`reset()`) passiert in `run()`s
Schleife nach `appl_exit`/`wind_new`, also nach der Abmeldung. Gleicher
Zustandsuebergang, ohne das Fenster, in dem ein Repaint eine freigegebene Arena
lesen koennte.

### 1.5 `sh_ldapp`s DESKTOP-Arm (`kernel/src/shell_loop.rs:292`)

`crate::desk::reset(); crate::desk::run();` — `reset()` ist das, was GEMDOS
EmuTOS abnimmt, wenn der Desktop-*Prozess* endet (`deskmain.c:2131-2133`,
`deskstart.S:57-59`). Der Doc-Kommentar von `desk::reset` und
`global::reset` ist entsprechend korrigiert (beide behaupteten „in-kernel call,
kann nicht neu starten").

### 1.6 `do_read_inf` — DESK14 §4 eingeloest

* `main::do_read_inf` ist jetzt wortwoertlich `deskmain.c:918-925`:
  `if (app_read_inf()) { restart_desktop = TRUE; done = TRUE; }`. Der
  in-place-Nachzieher (`cnx_get_prefs` + `install_shortcuts` + `win_view`/
  `app_blddesk`/`win_srtall` + `win_bdall` + `redraw_desktop`) ist weg — den
  macht jetzt der Neustart aus `deskmain`s Prolog.
* `inf::app_read_inf` schreibt die gelesene Datei jetzt in den
  **AES-Shell-Puffer** statt sie anzuwenden:
  `shel_get(buf, CPDATA_LEN); shel_put(buf, CPDATA_LEN+rc+1);`
  (`deskapp.c:1530-1534`), und liest hoechstens `MAX_INF_TEXT` Bytes
  (`SIZE_SHELBUF-CPDATA_LEN-1`, `:1519`) statt der eigenen 64-KiB-Grenze. Beide
  Doc-Abweichungen sind gestrichen, nicht umformuliert.

### 1.7 Drei Begruendungen, die durch U3 falsch wurden — alle drei korrigiert

[[justification-is-copied-to-several-places]]: `grep` nach `gap G1` hat drei
Stellen gefunden, die behaupten, der Desktop starte am `shel_write` vorbei:
`kernel/src/launch.rs:124`, `tos-api/src/aes/shell.rs:378` und
`tos-api/src/aes/window.rs:4514`. Der Code dort bleibt richtig (die
`\AUTO\`-Programme haben wirklich kein `shel_write` davor — `bios/bios.c:713-714`),
die Begruendung war es nicht mehr. Dazu `desk/pro.rs` (`pro_run`s `(wh, curr)`-
Luecke ist jetzt erreichbar und damit eine echte, benannte Luecke) und
`desk/dir.rs:1674` (`fun_drag` gibt jetzt `exit_desktop` wirklich zurueck).

---

## 2. Die Zahlen

| | |
|---|---|
| **Driftprobe** | **38** — vorher **47** |
| Bauen `--board rpi4`, frisch | tos-api **64** / kernel **94** (Basis 64/94, **Delta 0**), rc 0 |
| Bauen `--board qemu-virt`, frisch | tos-api **64** / kernel **94** (Basis 64/94, **Delta 0**), rc 0 |
| clippy, beide Feature-Saetze | tos-api **80** / kernel **125** (Basis 80/125, **Delta 0**) |
| clippy-Treffer in `desk/`, `shell_loop.rs`, `launch.rs` | **0** |
| `cargo run -p xtask -- test` | **956 / 0 / 1** (Basis 956/0/1) |
| `fsck_msdos -n`, fuenf Abbilder | Phase 1–3 ohne Befund, rc 0 |

### Die Driftzahl, verrechnet

| Gruppe | 47 | 38 | Differenz |
|---|---:|---:|---|
| A `tos_api::` in `desk/` | 21 | 20 | −1 `tos_api::host::host()` (mit `console_pause_for_key`) |
| B `crate::` in `desk/` | 21 | 12 | −6 `crate::shell_loop`, −2 `crate::launch`, −1 `crate::fbcon` |
| C Kernel → `desk` | 5 | 6 | **+1** `crate::desk::reset()` in `sh_ldapp` |

Die `shell_loop`- und `launch`-Gruppen aus §2.2 D sind **weg**, wie U3
verlangt. Die eine Steigerung ist der Aufruf, den U3 selbst vorschreibt; U4s
Abnahmekriterium listet `desk::reset()` ausdruecklich als Ueberlebenden, daher
bleibt `kernel/src/lib.rs:475` (A106) unberuehrt.

---

## 3. Was nachgewiesen ist (headless)

Alles in `measurement-2026-09-26-desk43-u3/`, QEMU `raspi4b`, 640×480,
`-display none`, QMP-Screendumps. **0** Treffer auf `EXC|PANIC|panic` in allen
Protokollen.

**Beweispunkt 4 — Driftprobe.** 47 → **38**, oben aufgeschluesselt
(`build/drift-probe.txt`).

**Beweispunkt 5 — AES-Trace des Doppelklicks.** Der IKBD-Injektor klickt
doppelt auf `C:\U3\ARGVTEST.NAP`. Serielle Kurzfassung:

```
[shell] sh_ldapp: starting C:\U3\ARGVTEST.NAP, sh_nextapp=0, sh_isgem=1
ARGVTEST: argc 1
[shell] sh_ldapp: 'C:\U3\ARGVTEST.NAP' exited with rc=0
[shell] sh_ldapp: starting NEWDESK, sh_nextapp=3, sh_isgem=1
[desk] desktop up (1 drives, 1 window(s) from the context)
```

Der svc-Trace (`trace/QEDTRACE-doubleclick.log`, Auszug in
`analysis/trace-doubleclick-excerpt.txt`) zeigt in dieser Reihenfolge:
`evnt_multi(25, clicks=2)` → `Dsetdrv`/`Dsetpath "C:\U3\"` → `graf_mouse(78)`
→ **`shel_write(121)`** → `evnt_button(21)` → `men_update` → `wind_get(WF_CXYWH)`
(`cnx_put`) → **`shel_get(122)`/`shel_put(123)`** (`app_save(FALSE)`) →
`menu_bar(30)` → `wind_set(105)` → **`appl_exit(19)`** → `appl_init(10)` des
neuen Desktops. **`Pexec` (`0x4b`) kommt in 744 Zeilen null Mal vor.**

> **Praezisierung zum Wortlaut des Plans.** „`shel_write(121)` ist die letzte
> Desktop-Aktion vor `appl_exit(19)`" stimmt woertlich **nicht** — und darf es
> nicht: dazwischen liegen genau die Schritte, die EmuTOS dort auch faehrt
> (`deskmain.c:1003,1006,2154,2155,2160,2163`). Richtig ist: `shel_write` ist
> die letzte Aktion der **Ereignisschleife**, `appl_exit` die letzte des
> **Epilogs**, und dazwischen steht `deskmain.c:989-2166` und sonst nichts.

**Beweispunkt 3a — `#Z 01 C:\HELLO2.NCP@`.** Gegen den Kernel `d79e71f` mit
demselben Abbild: identische Seriellfolge, Desktopbild danach **0 Pixel**
verschieden.

**Beweispunkt 3b — A106-Restart.** Der TeraDesk-Aufbau aus
`measurement-2026-09-22-a106` (zwei Aufloesungswechsel hintereinander): zwei
`AES restart`-Runden, und alle sechs Vergleiche **0 Pixel** (Kaltstart vs.
Neustart 1 vs. Neustart 2, sowie die drei Dialogstufen gegeneinander).

**Bootbilder ohne Regression.** U3 gegen `d79e71f`, beide INF-Zustaende
(Platzhalter und Fenster): je **0 Pixel**.

**Zweites Board.** `--board qemu-virt` bootet bis `[desk] desktop up
(0 drives, …)`, 0 Panics.

**Zugabe: das, was von Beweispunkt 1 headless geht.** Der Desktop **vor** dem
Doppelklick gegen den **neu gebauten** Desktop nach dem Programmlauf:
**0 abweichende Pixel** — Fenster `C:\U3\` mit Rechteck, Pfad, Infozeile und
Inhalt wieder da, rekonstruiert aus `shel_put`/`shel_get` (U2) + `cnx_get`,
nachdem `desk::reset()` die Struktur dazwischen fallen gelassen hat. Dasselbe
fuer ein Konsolenprogramm (`.NCP`, `sh_isgem=0`): **0 Pixel**.

---

## 4. Was die echte Pi 4 beweisen muss (Punkte 1 und 2) — Anweisung

Vorbereitung (einmal):

```
tools/make-sd-image.sh /pfad/sd.img 64      # einzige zulaessige Quelle
cargo run -p xtask -- build --board rpi4 --release
```

`kernel8.img` und das Abbild auf die Karte, Pi 4 mit Tastatur, Maus und Monitor
starten. **Kein `#Z` in `C:\NEWDESK.INF`**, damit der eingebaute Desktop
hochkommt.

**Punkt 1 — Start und Rueckkehr einer GEM-Anwendung.**

1. Doppelklick auf das Laufwerkssymbol `C` → Wurzelfenster.
2. Zwei, drei Fenster oeffnen, verschieben, in der Groesse aendern — ein
   Zustand, den man wiedererkennt.
3. Nach `C:\Apps\QED\` navigieren und `QED.NAP` doppelklicken.
   * **Erwartet:** Sanduhr, der Bildschirm wird geraeumt, QED kommt im
     **Vollbild** mit **eigener** Menuleiste
     ([[singletasking-app-owns-full-screen]]).
   * **Falsch waere:** QED erscheint *ueber* dem noch sichtbaren Desktop, oder
     die Desktopfenster stehen als tote Pixel daneben.
4. QED beenden (*File → Quit*).
   * **Erwartet:** der Desktop kommt zurueck **mit genau denselben Fenstern**,
     an denselben Stellen, mit denselben Pfaden und Scrollstaenden. Serielle
     Zeile: `[shell] sh_ldapp: starting NEWDESK, sh_nextapp=3` und danach
     `[desk] desktop up (N drives, M window(s) from the context)` mit **M =
     Anzahl der offenen Fenster** vor dem Start.
   * **Falsch waere:** ein leerer Desktop ohne Fenster (dann traegt der
     Shell-Puffer nicht), ein haengender Bildschirm, oder ein zweiter
     Mauszeiger.
5. Denselben Doppelklick ein zweites und drittes Mal — der Zustand muss jedes
   Mal gleich zurueckkommen (die Schleife laeuft jetzt bei **jedem** Start
   durch `appl_exit`, nicht mehr nur einmal).

**Punkt 2 — *File → Execute EmuCON*.**

1. Aus dem Desktop *File → Execute EmuCON* waehlen.
   * **Erwartet:** der Mauszeiger verschwindet, der Bildschirm wird zum
     VT52-Textschirm, die EmuCON-Eingabeaufforderung steht oben links.
     Seriell: `[shell] sh_ldapp: starting …, sh_nextapp=1` (`CONSOLE_APP`).
   * **Falsch waere:** ein Textfenster *im* GEM-Bild, ein stehen gebliebener
     Mauszeiger als schwarzer Fleck, oder gar kein Bildwechsel.
2. Ein, zwei Kommandos tippen, dann die Konsole beenden.
   * **Erwartet:** Sanduhr, der Desktop wird komplett neu gezeichnet und steht
     wieder mit seinen Fenstern da; der Pfeil ist zurueck.
3. Screendump/Foto von der Konsole **und** vom Desktop danach.

**Noch eine Sache, die nur auf der Pi geht** (bitte mitpruefen, sie ist die
sichtbare Folge von §6): ein `.NCP` aus dem Desktop doppelklicken (z. B.
`C:\SYSINFO.NCP`). Seine Ausgabe bleibt **nicht** mehr stehen — der Desktop
kommt sofort darueber. Wenn das stoert, ist das die Entscheidung aus §6, nicht
ein Fehler.

---

## 5. Massnahme 3 aus §6.4 — jede beruehrte `fn` mit ihrer Quelle

Keine Ausnahme; die Liste ist vollstaendig.

| `fn` | EmuTOS |
|---|---|
| `desk::run` | `desk/deskstart.S:52-56` |
| `desk::deskmain` | `desk/deskmain.c:1938-2172` |
| `desk::close_desktop_windows` | `desk/deskmain.c:1921-1930` |
| `desk::wind_new` (aesbind) | `desk/gembind.c:827-831`, AES `gemwmlib.c:1875-1893` |
| `desk::reset` | `deskmain.c:2131-2133`, `deskstart.S:57-59`, `deskmain.c:2140-2147` |
| `global::reset` | dieselben |
| `main::clear_restart_desktop` / `main::restart_desktop` | `deskmain.c:359`, `:1946`, `:2136` |
| `main::pro_run_native` | `desksupp.c:955-986` |
| `main::isgraf` | `desksupp.c:876`, `deskapp.h:28`, `deskapp.c:181-188,311-322` |
| `main::open_item` | `desksupp.c:1190-1280` |
| `main::open_document` | `desksupp.c:924-951` |
| `main::do_aopen_drop` | `desksupp.c:843-1031`, `deskfun.c:967` |
| `main::do_open_selected` | `deskmain.c:666-668` |
| `main::hndl_button` (Doppelklick-Arm) | `deskmain.c:986-1003` |
| `main::do_filemenu` / `main::do_optnmenu` / `main::hndl_menu` | `deskmain.c:654`, `:836`, `:1012-1039` |
| `main::do_read_inf` | `deskmain.c:918-925` |
| `main::do_execcli` | `deskmain.c:750-770` |
| `inf::app_read_inf` | `deskapp.c:1492-1546` |
| `shell_loop::sh_ldapp` (DESKTOP-Arm) | `gemshlib.c:600-612` |

---

## 6. Entscheidung, die dem Anwender gehoert: `console_pause_for_key`

`console_pause_for_key` (`main.rs`, „[ Press any key to return to the desktop ]")
hatte genau **einen** Aufrufer: den Console-Arm von `launch_native`. Den
loescht U3 laut Auftrag. Eine neue Heimat gaebe es nur in `sh_ldapp` — also
eine **neue** Umgehung im Shell-Loop, die EmuTOS nicht hat (`sh_chgrf` schaltet
direkt zurueck, `gemshlib.c:513-523`). Nach der Ordnungsregel (§6.3: keine
Umgehung stehen lassen, „weil es sonst nicht laeuft") ist es damit weg.

Das ist **STOP-3 des alten Plans, und U4 besitzt die Entscheidung** — sie faellt
hier nur frueher an, weil der Traeger verschwindet. Sichtbare Folge:
`screendumps/n-clicked.png` — 1,5 s nach dem Doppelklick auf ein `.NCP` ist der
Desktop schon wieder da, die Programmausgabe ist fort. Mit dem Wegfall gehen
auch die letzten beiden Umgehungen `crate::fbcon::write_byte` und
`tos_api::host::host()` (Gruppe C des Bypass-Registers, §2.2) — das ist
gleichzeitig der Grund, warum die Driftprobe bei 38 und nicht bei 40 steht.

**Will der Anwender die Pause behalten, ist sie eine benannte, in `docs/api/`
einzutragende Abweichung und braucht einen EmuTOS-fremden Traeger.** Dann
zurueckbauen, nicht nachtraeglich einschmuggeln.

---

## 7. Offene / gemeldete Punkte

1. **`.NTP` und `isgraf`.** `super::ftype` nennt als m68k-Analogon von `.NTP`
   *„`.TTP` / `.GTP`"* — und die beiden haben **verschiedene** `AF_ISCRYS`
   (`#P *.TTP` ohne, `#Y *.GTP` mit, `deskapp.c:181-188`). Ich habe das
   heutige Verhalten erhalten (`.NTP` → `isgraf = TRUE`, also `.GTP`), weil der
   geloeschte `launch_native` ein `.NTP` genauso wie ein `.NAP` behandelt hat.
   **Widerspruch im eigenen Baum:** `main::default_flavor` bildet
   `Params → AppFlavor::Ttp` ab, also auf die andere Haelfte. Eine Zeile, eine
   Entscheidung — gehoert gestellt, nicht geraten.
2. **`opn_appl` fehlt weiter** (`deskpro.c:71-121`, der Parameterdialog fuer
   `.TTP`/`.GTP`): ein `.NTP`-Doppelklick startet mit leerem Tail. Unveraendert
   benannt, jetzt im Doc von `pro_run_native`.
3. **`pro_run`s `(wh, curr)`** und `do_wopen` (`desksupp.c:294`) fehlen weiter.
   Der Zweig war bisher unerreichbar, seit U3 ist er erreichbar — damit ist es
   eine echte Luecke statt einer Formalie. Kein Stub gebaut.
4. **`tos_api::aes::window::wm_new_keep_desktop`** hat seit DESK35 keinen
   Aufrufer und ist seit U3 auch konzeptionell erledigt. Oeffentliche
   AES-Flaeche zu entfernen ist eine eigene Stufe — nur der Doc-Kommentar ist
   korrigiert.

## Out-of-scope-Beobachtungen (unberuehrt, je eine Zeile)

* `kernel/src/lib.rs:475` ruft `desk::reset()` nach `aes::reinit()`; seit U3 ist
  das redundant (der DESKTOP-Arm resettet ohnehin) — U4s Abnahmekriterium
  listet es aber weiter, deshalb stehen gelassen.
* `open_document` waehlt `isgraf` ueber die Dateiendung der Anwendung, EmuTOS
  ueber `pa->a_flags` des installierten `ANODE` (`desksupp.c:876,1001`) — eine
  Abweichung, die aelter ist als U3.
* `hndl_button` ruft `men_update()` nach dem Doppelklick auch dann, wenn
  `done` gesetzt ist; EmuTOS tut dasselbe (`deskmain.c:1006`), aber es sind 39
  AES-Traps unmittelbar vor `appl_exit` — auffaellig im Trace, konform.
* `do_deskmenu` gibt `()` zurueck, EmuTOS `WORD done` (`deskmain.c:612`); der
  einzige Zweig (`ABOUITEM`) setzt es nie.
