# A62 + A56 — AES-Prozesstabelle und `ap_id`-Vergabe, gemeinsam untersucht

Auditor, 2026-09-20. **Untersuchung, kein Fix.** Kein Produktcode geändert
(`git status --short` leer vor und nach der Arbeit bis auf die neuen Artefakte;
`kernel/src/host_env.rs` sha256 `287ff741…` vorher = nachher). Sonden liefen
in einer Scratch-Kopie des Baums; Messartefakte:
[`measurement-2026-09-20-a62/`](../../measurement-2026-09-20-a62/RUNINFO.TXT).

Basis: `desk-rebuild` @ `8f052b2`. EmuTOS: `references/emutos` @
`1a7484aba1bfba23e2c0899e243b6d8177fe41df`, Zeilennummern selbst nachgeprüft.

---

## 0. Ergebnis in vier Sätzen

1. **A62 Punkt 1 — die Zahl:** Die Tabelle bricht **nicht** bei acht
   verschiedenen Programmen nacheinander (neun gefahren: alle bekommen pid 1,
   ein Deskriptor wird neunmal umbenannt), sondern bei **Verschachtelungstiefe 9**
   (`Pexec` in `Pexec` …, Slot 8 = pid 9). Gemessen in QEMU raspi4b und auf dem
   Host, bitgleich. Die Prämisse des Tickets („nicht gebunden, wenn mehrere
   verschiedene Programme nacheinander laufen") ist **widerlegt** — der
   Schlüssel ist pid = Slot + 1, nicht der Name.
2. **A62 Punkt 2:** Ja, `appl_init` liefert heute noch `-1` bei voller
   Tabelle (Tiefe 9–16 gemessen); EmuTOS' `ap_init` kann das nie
   (`aes/gemaplib.c:54-63`, drei Anweisungen ohne Fehlerpfad). Das Programm in
   Tiefe 9 läuft weiter, ist aber **taub**: `appl_write` an es liefert 0, jede
   `WM_*`/`MN_SELECTED` wird verworfen, `evnt_multi(MU_MESAG)` feuert nie.
3. **A56 — der Wert:** EmuTOS gibt **pid 0** dem Desktop, **jeder** vom Shell-
   Loop gestarteten Anwendung **und jedem per `Pexec` gestarteten Kind** — sie
   laufen alle auf demselben `rlr` (AESPD 0). NewTOS gibt 1 (Desktop, Slot 0),
   2 (Kind in Slot 1), 3 … Kein portiertes Programm (TeraDesk, QED, cflib)
   vergleicht `ap_id` mit der Konstanten 0 oder `msg[1]` mit `gl_apid` für
   AES-eigene Nachrichten — **A56 bleibt P2**. Aber: A56 lässt sich nicht
   „einfach auf 0 setzen", weil A39 die Ressourcen **über genau diese pid**
   schlüsselt und Eltern/Kind dann kollidieren (H05 kehrte zurück).
4. **Gemeinsamer Vorschlag:** die **Identität** (Deskriptor, Ressourcen,
   Wartekey) über den GEMDOS-Slot führen — dann ist die Tabelle per Bauart
   `MAX_PROCESSES + 1` groß und A62 verschwindet — und den **berichteten
   Wert** `ap_id`/`msg[1]`/`WF_OWNER`/`appl_find` davon getrennt nach EmuTOS
   berechnen (0 für die Anwendungskette, 1 für den Screen-Manager-Absender).
   Reihenfolge: erst Identität (A62), dann Wert (A56). Abschnitt 6.

---

## 1. Was gemessen wurde (A62 Punkt 1 und 2)

Zwei Sonden, beide über den echten `svc`-Pfad in QEMU `raspi4b` (headless,
Release-Kern aus der **unveränderten** Scratch-Kopie, sha256 `2a08ccad…`) und
noch einmal über den echten LP64-Dispatcher auf dem Host
(`host/zz_a62_probe.rs.txt`, Prozessmodell = das `Proc`-Fixture der
`resource.rs`-Tests). Alle Zahlen sind auf beiden Wegen identisch.

### 1a. Neun **verschiedene** Programme nacheinander (`SEQ1.NAP` … `SEQ9.NAP`)

`#Z`-Autorun startet `SEQ1.NAP`; jedes kettet per `shel_write(SHW_EXEC)` auf
das nächste (der A29-Kettenweg, `[shell] sh_ldapp: starting C:\SEQn.NAP`).
`serial/seq-serial.log:42-101`:

    SEQTST n=1  cmd=[C:\SEQ1.NAP] appl_init=1 appl_find(self)=1 appl_find(prev)=-99
    SEQTST n=2  cmd=[C:\SEQ2.NAP] appl_init=1 appl_find(self)=1 appl_find(prev)=-1
    …
    SEQTST n=9  cmd=[C:\SEQ9.NAP] appl_init=1 appl_find(self)=1 appl_find(prev)=-1
    [desk] desktop up (1 drives)

**Nie −1.** Alle neun landen in GEMDOS-Slot 0 (`[launch] EL0: slot 0`), pid 1,
`pd::register(1, path)` benennt **denselben** Deskriptor um
(`tos-api/src/aes/pd.rs:603-620`: erst Suche nach `in_use && pid == pid`, nur
sonst ein freier Slot). Deshalb `appl_find(vorgänger) = -1`: der Name des
Vorgängers ist überschrieben, nicht ein zweiter Eintrag belegt. Das ist
EmuTOS' Verhalten (`sh_ldapp` benennt das eine `rlr` um,
`aes/gemshlib.c:633`), nur mit dem Wert 1 statt 0.

### 1b. Verschachtelt: jedes Programm `Pexec`t sich selbst, 16 tief (`NESTTST.NAP`)

Das ist der TeraDesk-Startweg — H05 hat gemessen, dass TeraDesk Kinder per
`Pexec(0)` aus EL0 startet (`startprg.c:312`), nicht über die Shell.
`serial/nest-serial.log:42-123`:

    depth=1  slot 0  appl_init=1   WF_OWNER=1   appl_write(self)=1
    depth=2  slot 1  appl_init=2   WF_OWNER=2   appl_write(self)=1
    …
    depth=8  slot 7  appl_init=8   WF_OWNER=8   appl_write(self)=1
    depth=9  slot 8  appl_init=-1  WF_OWNER=9   appl_write(self)=0     <-- hier
    …
    depth=16 slot 15 appl_init=-1  wind_create=-1 (Fenstertabelle voll, anderes Thema)
    depth=16 Pexec(child) returned -39                                  <-- ENSMEM, MAX_PROCESSES
    … alle 16 appl_exit=1 -> Pterm, dann [desk] desktop up (1 drives)

**Die Zahl für A62: 9 verschachtelte Prozesse.** `MAX_APPS = 8` gegen
`MAX_PROCESSES = 16` heißt: die pids 9–16 können nie einen Deskriptor bekommen.
`wind_create` funktioniert weiter und trägt `w.owner = 9` ein — ein Fenster,
dessen Besitzer keine Warteschlange hat; jede `send_redraw` dafür verwirft
`post_msg` (`pd.rs:362-367`, `by_pid` → `None`).

Nebenbefund derselben Messung, nicht verfolgt: `appl_find("NESTTST ")` liefert
in **jeder** Tiefe 1 — nur der Kernel-Startpfad (`launch.rs:102`) ruft
`register_process`; der EL0-`Pexec`-Pfad (`exceptions.rs::handle_pexec`) benennt
den Deskriptor des Kindes **nicht** (Name bleibt acht Leerzeichen). EmuTOS
benennt bei einem `Pexec`-Kind ebenfalls nichts (kein `p_nameit` außerhalb von
`sh_ldapp`/`pstart`), stimmt also — mit dem Unterschied, dass es dort gar
keinen zweiten Deskriptor gibt.

### 1c. `register_process` bei voller Tabelle

Host-Sonde: `register_process(8, "C:\NINTH.NAP")` bei acht belegten
Deskriptoren → `appl_find("NINTH   ") = -1`. `pd::register` gibt still auf
(`pd.rs:615`, `None => return`). Der Name eines neunten Programms geht also
ohne Meldung verloren.

---

## 2. Das EmuTOS-Modell, aus der Quelle

| Frage | EmuTOS | Beleg |
|---|---|---|
| Wie viele AESPDs? | `NUM_PDS = NUM_ACCS + 2 = 8` als **statisches Maximum**; **live** `totpds = num_accs + 2` — ohne Accessories genau **2** | `aes/struct.h:36`, `include/sysconf.h:37`, `aes/geminit.c:819` |
| Wer bekommt welche pid? | Index = pid, fest bei Init: `rlr->p_pid = i` für alle; danach `curpid = 0; rlr->p_pid = curpid++` → **pid 0 = der Anfangsprozess**, der zu `accdesk_start` → `sh_main` → Desktop/Anwendung wird | `aes/geminit.c:851-876` |
| Screen Manager | `ctl_pd = iprocess("SCRENMGR", ctlmgr)` → `pstart` → `getpd` → `pd_index(1)`, `p_pid = 1` | `aes/geminit.c:884-888`, `aes/gempd.c:60-70,110-131` |
| Accessories | `load_one_acc` → `pstart` → pids 2 … | `aes/geminit.c:236-256` |
| Wer sonst bekommt einen AESPD? | **Niemand.** `getpd` hat genau zwei Aufrufer (`iprocess`, `load_one_acc`). Eine per `sh_ldapp`/`dos_exec` gestartete Anwendung läuft auf `rlr` (= AESPD 0, umbenannt); ein per **`Pexec` gestartetes Kind** läuft ebenfalls auf `rlr` — `Pexec` ist ein GEMDOS-Aufruf, die AES sieht ihn nicht | `aes/gemshlib.c:633-645`, `aes/gempd.c` (Aufrufer von `pstart`: nur `geminit.c:226,255`) |
| Was liefert `appl_init`? | `rlr->p_pid` — **0** für Desktop, jede Anwendung, jedes Kind; nie −1 | `aes/gemaplib.c:54-63`, `aes/gemsuper.c:94` |
| Lebendprüfung bei der Zielsuche? | keine: `fpdnm` läuft über `totpds` Einträge und vergleicht nur `p_pid` bzw. `p_name` | `aes/gempd.c:38-57`, Aufruf aus `APPL_WRITE`/`APPL_READ` `aes/gemsuper.c:102-105` |
| `msg[1]`? | `ap_msg[1] = rlr->p_pid` = **der Absender**. Control-Manager-Nachrichten (`ct_msgup`: `WM_CLOSED/FULLED/MOVED/SIZED/TOPPED/UNTOPPED/HSLID/VSLID`, `MN_SELECTED`, `AC_OPEN`; `handle_arrow_msg`: `WM_ARROWED`) werden aus `ctlmgr` gesendet → **`msg[1] = 1`**. `WM_REDRAW` aus `w_redraw` kommt aus dem `wind_*`-Aufruf der Anwendung → **`msg[1] = 0`**. Die von `ev_mesag` synthetisierte `WM_ARROWED` trägt `rlr->p_pid` des Lesers (0) | `aes/gemwmlib.c:851-862,897-905`, `aes/gemctrl.c:82-84,125,314,357-364`, `aes/gemevlib.c:102-111` |
| `WF_OWNER`? | `pwin->w_owner->p_pid` — der Erzeuger (`w_setup(rlr, …)`), also 0 für jedes Anwendungsfenster | `aes/gemwmlib.c:1579-1580` |

Zwei Folgerungen, die für den Umbau tragen:

- **EmuTOS' Tabelle ist nicht „8 Deskriptoren für Anwendungen"**, sondern
  „1 Anwendungsprozess + 1 Screen Manager + n Accessories". Die Anwendungskette
  (Desktop → Anwendung → Kind → Kindeskind) hat **einen** Deskriptor, egal wie
  tief. NewTOS' `MAX_APPS = 8` ist eine eigene Erfindung mit anderem Sinn.
- Innerhalb dieser einen Kette ist die **Warteschlange geteilt** und das ist
  beobachtbar folgenlos, weil der Elternprozess während des `Pexec` blockiert
  ist und `ap_exit`/`sh_ldapp` die Schlange vor der Rückkehr leeren — genau die
  Grenze, die A28/A29 nachgebaut haben.

---

## 3. NewTOS heute — jede Stelle, die pid-Werte vergibt, vergleicht oder hinausreicht

| Stelle | Was passiert | EmuTOS-Wert | NewTOS-Wert |
|---|---|---|---|
| `tos-api/src/aes/appl.rs:166-174` `current_pid` | `slot_for(bp) + 1`, Kernel-Kontext → 1 | `rlr->p_pid` = 0 (Kette), 1 (SCRENMGR), 2+ (ACC) | Desktop 1, Slot 0 → 1, Slot 1 → 2 … Slot 15 → 16 |
| `appl.rs:218-245` `appl_init` → `ap_id`, `int_out[0]` | `pd::open(pid)`; **−1 wenn kein Slot** | 0, nie −1 | 1…8; −1 ab pid 9 (gemessen) |
| `appl.rs:257-262` `appl_exit` | `pd::close(current_pid())` | `ap_exit` auf `rlr` | dito, je pid |
| `appl.rs:266-283` `appl_find` → `int_out[0]` | `pd::find_by_name` → Deskriptor-pid | 0 für Anwendungsnamen (die Kette), 1 für `"SCRENMGR"`, 2+ ACC | Deskriptor-pid (1, 2, …); `"SCRENMGR"` → −1 |
| `appl.rs:304-337` `appl_write(AP_RWID)` | `by_pid(target)`; **0 wenn kein Deskriptor** | `fpdnm(NULL, pid)`, nie ohne Ziel innerhalb `totpds` | 0 ab pid 9 (gemessen) |
| `appl.rs:347-379` `appl_read(AP_RWID)` | `by_pid(id)` | dito | dito |
| `event.rs:116` `MN_SELECTED msg[1]` | `= current_pid()` (der Leser) | **1** (`ctl_pd`) | eigene pid |
| `window.rs:1594` `send_redraw msg[1]` (`WM_REDRAW`) | `= current_pid()` | **0** (die Anwendung selbst) | eigene pid |
| `window.rs:3936` `intercept msg[1]` (`WM_MOVED/SIZED/CLOSED/FULLED/TOPPED/ARROWED/…`) | `= current_pid()` | **1** (`ctl_pd`) | eigene pid |
| `window.rs:4079` `WM_UNTOPPED msg[1]` | `= current_pid()` | **1** | eigene pid |
| `window.rs:2394` `wind_create` → `w.owner`; `:2897` `WF_NEWDESK` | `= current_pid()` | `rlr` (0) | eigene pid |
| `window.rs:2818` `wind_get(WF_OWNER)` → `int_out[1]` | `w.owner` | 0 | 1, 2, … |
| `resource.rs:678` `caller_pid` — **Schlüssel der Ressourcenliste** (A39) | `= current_pid()` | je Anwendung eigene `AESGLOBAL` (`rs_sglobe`), pid **nicht** der Schlüssel | pid |
| `bdos/process/pterm.rs:178` `on_process_exit(pid_of_slot)` | löscht alle Einträge **dieser pid** | — | pid |
| `pd.rs:409-411` `wake_mesg_waiter` / `sched::wait_mesg(pid)` | Wartekey der Schlange **= pid** | — | pid |
| `mod.rs:659` `pid_of_slot`, `:669` `register_process`, `:649` `close_client_if_open`; `launch.rs:102,137` | Kernel-Startpfad, nur dort | `sh_ldapp` auf `rlr` | Slot 0 → pid 1 |
| `exceptions.rs::handle_pexec` (EL0-`Pexec`) | **kein** `register_process`, kein `close_client_if_open` | kein `p_nameit`, keine Nachsorge (nur `sh_ldapp` hat sie) | Deskriptor entsteht erst bei `appl_init`, Name leer |
| `menu.rs:858` `menu_register(pid)` | nur `pid >= 0` | `mn_register`: nur `pid >= 0` | gleich |
| `kernel/src/desk/mod.rs:650` `desk::run` → `appl_init` | Kernel-Kontext | `deskmain.c:1953` → 0 | **1** |
| `kernel/src/selftest_sleepwake_switch.rs:71` | annotiert „Slot + 1" | — | unverdrahtet (bekannt) |
| `trace.rs:718` | nur Trace | — | — |

**Der eingebaute Desktop und das Kind nach H05:** `desk::run` → `appl_init` im
Kernel-Kontext → **1**. Das per Doppelklick gestartete Programm (`run_gem_app` →
`launch.rs`) → Slot 0 → **1**, derselbe Deskriptor (A29). Startet **dieses**
Programm ein Kind per `Pexec` (TeraDesk → QED, H05 `run1`/`run3`), bekommt das
Kind Slot 1 → **2** (H05-Trace `[aes] pid=2 op 10 … out=(2,…)`). EmuTOS:
0, 0, 0.

---

## 4. A62 Punkt 3 — „Tabelle über den GEMDOS-Slot indizieren" gegen den heutigen Code

`current_pid() = slot + 1` **ist** schon eine Slot-Indizierung des Werts; die
8er-Tabelle in `pd.rs` sucht diesen Wert dann linear (`by_pid`). Was an der
Tabelle hängt (vollständig, per Sweep):

- **Nachrichtenschlange je Deskriptor** (`MsgQueue`) — `post_msg`/`post_words`,
  `appl_read`, `evnt_mesag`, `evnt_multi` (`by_pid`).
- **Name** (`p_name`) — `register` (Kernel-Start) schreibt, `appl_find` liest.
- **Flags** — `open` (`|= AP_OPEN`), `close`/`close_if_open` (`&= !AP_OPEN`),
  `register` (`= 0`, A28), `appl_read`/`event.rs` (`|= AP_MESAG`).
- **Wartekey** `wait_mesg(pid)` — in `sched`, außerhalb der Tabelle, aber mit
  demselben Schlüssel.
- **Ressourcenliste** — `RscEntry.pid` (A39), außerhalb der Tabelle, derselbe
  Schlüssel.
- **Fensterbesitz** — `Window.owner`, derselbe Wert, wird durch `WF_OWNER`
  hinausgereicht und durch `post_to_owner`/`send_redraw` als Ziel benutzt.

### Der Umbau (Identität = Slot)

1. `pd.rs`: `TABLE: [Aespd; MAX_PROCESSES + 1]`, Index = GEMDOS-Slot, Index
   `MAX_PROCESSES` = Kernel-Kontext (Desktop). `in_use`/`pid`-Suche entfällt;
   `open`/`register`/`close`/`by_*` nehmen einen **Slot-Index**. Damit gibt es
   per Bauart genau einen Deskriptor pro Prozess; `open` kann nicht mehr
   scheitern → `appl_init` verliert den `-1`-Pfad (A62 Punkt 2 erledigt sich).
2. `appl.rs`: `current_pid()` wird zu `current_slot()` (Identität) — der
   **berichtete** Wert wird eine eigene Funktion (Abschnitt 5/6).
3. `resource.rs`: `caller_pid()` → Slot; `on_process_exit(pid_of_slot(slot))`
   → `on_process_exit(slot)`; `RscEntry.pid` → `slot` (A39 bleibt in der Sache
   unverändert, nur der Schlüsseltyp wechselt; `0 = frei` muss zu einem
   Sentinel werden, weil Slot 0 gültig ist).
4. `sched::wait_mesg(pid)` → `wait_mesg(slot)`; `pd::wake_mesg_waiter` folgt.
5. `window.rs`: `Window.owner` = Slot (Identität); `WF_OWNER` und `msg[1]`
   berichten den **Wert** aus der Übersetzung.
6. `mod.rs`: `pid_of_slot` verschwindet oder wird zur Wert-Übersetzung;
   `register_process(slot, …)`/`close_client_if_open(slot)` bleiben (A28
   unverändert: `register` setzt weiter Name + `flags = 0`).
7. `kernel/src/shell_loop.rs:341-357`: Punkt 10 der Lückenliste streichen
   bzw. auf „Wert-Übersetzung" umschreiben.

### Was bricht (Tests und Selftests, per Sweep)

- `pd.rs` Unit-Test `reopening_preserves_flags_and_queue_even_when_the_table_is_full`
  (arbeitet mit negativen Test-pids und `open(-200).is_none()`): entfällt oder
  wird zu „Slot bleibt derselbe".
- `tos-api/tests/aes_appl_init.rs` (A25), `aes_app_return_descriptor.rs`
  (A29), `aes_launch_boundary_reset.rs` (A28): rufen `register_process(0, …)`,
  `close_client_if_open(pid_of_slot(0))`, vergleichen `child_pid == desk_pid`
  (die A29-Prämisse „Slot 0 teilt den Deskriptor des Desktops"). **Diese
  Prämisse fällt weg**, wenn der Kernel-Kontext einen eigenen Index bekommt —
  und muss dann entschieden werden (Abschnitt 6, Punkt „Desktop-Identität").
- `resource.rs`-Tests (`Proc::pid()`, `on_process_exit(self.pid())`),
  `aes_global_layout.rs` und `aes_appl_init.rs` vergleichen `global[2]` nur mit dem zurückgegebenen Wert (wertneutral, bleiben grün), `kernel/src/selftest_aes.rs`,
  `selftest_sleepwake_switch.rs` (unverdrahtet), `trace.rs` (`r.pid`).
- Eingefrorene Messtabellen: Fall 644 (`aes-r4-subfunctions.tsv`, CRC
  `227422B2`) wird erst mit der **Wert**-Änderung (A56) grün; die Identitäts-
  Änderung allein ändert keinen gemessenen Wert.

**Aufwand:** eine Änderung in `pd.rs` (Tabelle + Signaturen), Folgeänderungen
in `appl.rs`, `event.rs`, `window.rs` (5 Stellen), `resource.rs` (Schlüssel),
`sched.rs` (Key), `mod.rs`, `pterm.rs`, `launch.rs`, `desk/mod.rs`; vier
Testdateien und zwei Selftests nachziehen. Kein Umbau von Warteschlange,
Flags, Namensfüllung oder A28-Reset.

---

## 5. A56 — was sich für uns konkret ändert

### 5a. App-Sweep (`/usr/bin/grep -rn "ap_id\|gl_apid\|_AESapid\|apid" sdk/apps/teradesk sdk/apps/qed/src sdk/cflib`)

| Fundstelle | Vergleich | Unter AES 0x0340 erreichbar? | Wirkung von pid 1 statt 0 |
|---|---|---|---|
| `teradesk/main.c:1821` `if ((ap_id = appl_init()) < 0) return -1` | nur `< 0` | ja | keine (bis Tiefe 9, dann Abbruch — A62) |
| `teradesk/va.c:463` `ap_id == dest_ap_id` (nach `appl_find(prgname)`) | eigene pid gegen `appl_find`-Ergebnis | ja, **bei jedem Programmstart** (`applik.c:1482` → `va_start_prg`) | heute konsistent (Kernel-Start benennt nur Slot 0 = TeraDesk selbst; Kinder haben leeren Namen → `appl_find` = −1 → Start). **Nach einem isolierten A56-Fix** (nur `ap_id` = 0, Kinder weiter mit eigenem Deskriptor + Name) würde `appl_find(QED)` = 0 = `ap_id` → Guard greift → Start — ebenfalls richtig. Kein Absturzpfad. |
| `teradesk/va.c:798,835,1002` `av_current == ap_id` (`av_current = message[1]`) | `msg[1]` gegen eigene pid | nur für AV-Protokoll-Nachrichten (`AV_*`, `VA_START`, `FONT_SELECT`), nie für `WM_*`/`MN_SELECTED` | keine |
| `teradesk/va.c:1169`, `window.c:558` `WF_OWNER != ap_id` | `int_out[1]` gegen eigene pid | `:1169` nur AV; `window.c:558` nur `mint \|\| geneva` | keine (und in NewTOS aus derselben Quelle, konsistent) |
| `teradesk/icon.c:469` `ap_id != owner` (`WF_OWNER` von Fenster 0) | | nur `aes_version >= 0x399` | keine |
| `qed/src/event.c:521` `id == msg[1]` | nur im Debug-Zweig mit `appl_search` (MagiC) | nein | keine |
| `qed/src/event.c:618-619`, `edit.c:1389`, `window.c:612`, `projekt.c:660` `appl_write(gl_apid, …)` an sich selbst | | ja | funktioniert mit jedem Wert, solange der eigene Deskriptor existiert |
| `cflib/appinit.c:40` `gl_apid = appl_init()` | **kein** `< 0`-Test | ja | QED in Tiefe 9 liefe mit `gl_apid = -1` taub weiter (A62) |
| `gemlib/overlay/mt_gem.h.in:931-938` `appl_write(msg, 16, 1) /* apid #1 is the SCREENMGR */` | Konstante **1** als Ziel | nur MagiC-`SM_M_SPECIAL`, von keiner unserer Apps benutzt | latent: in NewTOS wäre pid 1 die Anwendung selbst |

**Kein Programm steht auf `ap_id == 0` oder auf `msg[1] == gl_apid` für
AES-Nachrichten. A56 bleibt P2.** Was ich *nicht* gefahren habe: die
AV-Protokoll-Pfade brauchen zwei laufende Anwendungen und sind bei uns
strukturell unerreichbar (single-tasking).

### 5b. Warum A56 nicht isoliert geht — die zwei Abhängigkeiten

1. **A39 schlüsselt Ressourcen über `caller_pid()` = `current_pid()`**
   (`resource.rs:678`). Bekämen Eltern (TeraDesk) und Kind (QED) beide `ap_id`
   0 **als Identität**, würde `on_process_exit(0)` beim `Pterm` des Kindes auch
   die Einträge des Elternprozesses löschen — H05 („alle 36 `rsrc_gaddr` des
   äußeren TeraDesk liefern 0") käme zurück. Deshalb: **Wert ≠ Identität.**
2. **H02 hat `ctl_pd` als Flag gebaut, nicht als Prozess.** EmuTOS' `msg[1]`
   ist nur deshalb von der eigenen pid unterscheidbar, weil der Control Manager
   ein eigener Absender mit pid 1 ist. Heute schreibt NewTOS überall
   `current_pid()` (= die Leser-pid). Ein isoliertes „`ap_id` = 0" macht
   `WM_MOVED`/`MN_SELECTED` dann zu `msg[1] = 0`, wo EmuTOS 1 sagt — eine
   **neue** Abweichung bei den Control-Manager-Nachrichten, während die
   `WM_REDRAW`-Abweichung (heute 1 statt 0) verschwände. Der Absenderwert muss
   am Poster stehen: `intercept` (einschließlich `WM_ARROWED`)/`menu_intercept`/`post_untopped`
   → 1; `send_redraw` aus `wind_*`, `appl_write` → der Aufrufer.

Kurios, gemessen nicht vermutet: **heute** stimmt `msg[1]` für
Control-Manager-Nachrichten an ein Slot-0-Programm zufällig (1 = 1), und ist
für `WM_REDRAW` falsch (1 statt 0). Nach einem isolierten A56-Fix wäre es genau
umgekehrt.

### 5c. Spec-Befunde (an den Researcher)

- `docs/api/aes/application.md:452-453`: „The AES pid is small (0/1 = system
  processes, 2+ = accessories/applications)" — **falsch**: 0 = die Anwendung
  (Desktop, jede Anwendung, jedes Kind), 1 = Screen Manager, 2+ = **nur**
  Accessories (`geminit.c:875-888`, `gempd.c:60-70`).
- `:194-197`: „The documented `-1` failure return is the contract for a
  NewTOS that *can* fail to allocate an application slot" — lizenziert die
  Abweichung; EmuTOS' Vertrag ist „nie −1".
- `:587`: „`ap_id` = the Phase-06 process id" — legt den Wert auf Slot + 1
  fest, im Widerspruch zu `:164` (`rlr->p_pid`) und Fall 644.

---

## 6. Gemeinsamer Vorschlag für A62 + A56 (ohne Code)

**Prinzip:** Identität und Wert trennen. Die Identität eines AES-Klienten ist
sein GEMDOS-Slot (oder der Kernel-Kontext); alles Interne (Deskriptor,
Schlange, Flags, Name, Wartekey, Ressourcenliste, Fensterbesitz) hängt daran.
Der nach außen berichtete Wert (`ap_id`, `appl_find`, `WF_OWNER`, `msg[1]`,
`appl_write`-Ziel) wird aus der Identität **berechnet** — nach EmuTOS.

**Reihenfolge**

1. **A62 zuerst — Identität = Slot** (Abschnitt 4). Kein beobachtbarer Wert
   ändert sich, alle Messtabellen bleiben, A28/A29/A39 bleiben in der Sache
   unverändert. Ergebnis: keine Obergrenze mehr unterhalb `MAX_PROCESSES`, kein
   `-1` aus `appl_init`, kein stilles Verlieren des Namens.
   *Offene Entscheidung dabei:* **Desktop-Identität.** Heute teilt der
   Kernel-Desktop den Deskriptor mit Slot 0 (A29 nennt das EmuTOS-treu, weil
   `sh_ldapp` das eine `rlr` umbenennt). Mit Slot-Identität bekäme der Desktop
   einen eigenen Index — dann verliert A29s Prämisse, aber der Desktop hätte
   nach der Rückkehr eines Programms **immer** seinen Deskriptor, ohne auf den
   Nichtlöschpfad angewiesen zu sein. Beide Fassungen sind vertretbar; ich
   empfehle den eigenen Index, weil er die spätere Verlegung des Desktops nach
   EL0 (er wird dann Slot 0) ohne Sonderfall vorbereitet.
2. **A56 danach — Wert nach EmuTOS.** `reported_pid(identity)`:
   Anwendungskette (jeder Slot, Kernel-Desktop) → **0**; Screen-Manager-Absender
   → **1** (nur als `msg[1]`-Stempel der Control-Manager-Poster, es gibt keinen
   Prozess); Accessories → 2+ (sobald es sie gibt). Umkehrung für die
   Ziel-Auflösung (`appl_write(0)`, `appl_read(0)`): pid 0 → der **aktuell
   laufende** Klient (innerster Prozess der Kette) — das ist beobachtbar genau
   EmuTOS' eine geteilte Schlange, weil Eltern während `Pexec` blockiert sind
   und A28/A29 die Schlange an der Grenze leeren. `appl_find(name)` → 0 für
   jeden Anwendungsnamen, −1 wenn kein Deskriptor den Namen trägt. `WF_OWNER`
   → 0 für Anwendungsfenster. Fall 644 wird damit grün; `msg[1]` stimmt für
   `WM_REDRAW` **und** für `WM_MOVED`/`MN_SELECTED`.
3. **Spec** (5c) vor Schritt 2 korrigieren, damit der Code gegen ein richtiges
   Soll geschrieben wird.

**Risiken, benannt**

| Für | Risiko | Warum beherrschbar |
|---|---|---|
| A39 (Ressourcen) | Schlüsselwechsel pid → Slot; `0 = frei`-Sentinel kollidiert mit Slot 0 | reine Typänderung, `on_process_exit` bleibt an `Pterm` gebunden; Tests mit `Proc`-Fixture ziehen nach |
| A28 (Grenz-Reset) | keines — `register`/`flags = 0`/Drain bleiben, nur an Slot statt pid | `aes_launch_boundary_reset.rs` prüft weiter dieselben drei Stufen |
| A29 (Deskriptor überlebt) | die Prämisse „Desktop = Slot 0" fällt bei eigenem Desktop-Index | der Test wird **strenger** (zwei Deskriptoren, beide überleben); die Wirkung — Desktop empfängt nach der Rückkehr — bleibt die Abnahme |
| H02 (`ctl_pd` als Flag) | `msg[1] = 1` muss am Poster gestempelt werden, ohne Prozess | drei Stellen (`event.rs:116`, `window.rs:3936` — deckt auch `WM_ARROWED` —, `window.rs:4079`); `send_redraw` bleibt Aufrufer |
| H05 (Rückkehr des Kindes) | keines, sobald Identität ≠ Wert | genau der Grund für die Trennung |
| Messtabellen | Fall 644 kippt von rot auf grün; `global[2]`-Rauschen in den R3/R4-Tabellen wird zu Treffern | erwünscht |

**Was ich bewusst nicht vorschlage:** einen Screen-Manager-*Prozess* zu bauen,
nur um pid 1 zu belegen (H02 hat das entschieden), oder `MAX_APPS` einfach auf
16 zu setzen (heilt die Zahl, nicht die doppelte Schlüsselung).

---

## 7. Was gefahren wurde, was nicht

**Gefahren:** neun verschiedene Programme sequentiell (QEMU + Host); 16 tief
verschachtelt (QEMU + Host) einschließlich `appl_write`/`WF_OWNER`/`appl_find`
in jeder Tiefe; `register_process` bei voller Tabelle (Host); der 17. `Pexec`
(ENSMEM); Rückkehr aller Ebenen und Desktop-Start danach (seriell
`desktop up`). App-Sweep über TeraDesk, QED, cflib, gemlib-Overlay.

**Nicht gefahren / nicht verifizierbar:** ein reales GEM-Programm in Tiefe 9
(die Taubheit ist aus `appl_write = 0` + `event.rs:597-616` hergeleitet, nicht
mit QED gezeigt — QED in Tiefe 9 zu bringen braucht acht laufende Eltern);
AV-Protokoll-Pfade (brauchen zwei AES-Klienten gleichzeitig); die
Bildwirkung nach der 16er-Kette (nur seriell `desktop up`, kein Screendump —
außerhalb des Auftrags).

---

## 8. Nebenbeobachtungen (nicht verfolgt, nichts geändert)

- `exceptions.rs::handle_pexec` ruft weder `register_process` noch
  `close_client_if_open`: ein per EL0-`Pexec` gestartetes Kind, das `appl_init`
  ohne `appl_exit` ruft und stirbt, hinterlässt `AP_OPEN` + Schlange im
  Deskriptor seines Slots; der nächste Mieter dieses Slots erbt beides (A28 für
  den `Pexec`-Pfad). EmuTOS hat dort ebenfalls keine Nachsorge (nur `sh_ldapp`),
  aber auch nur *einen* Deskriptor, dessen Rest der Elternprozess sieht.
- `wind_create` liefert in Tiefe 16 `-1` — die Fenstertabelle hat 16 Einträge
  inklusive Desktop; jedes Kind hielt ein Fenster. Erwartbar, eigenes Thema.
- `DESKWH`-Besitzer: EmuTOS' `wm_init` ruft `w_setup(rlr, DESKWH, NONE)`
  (`aes/gemwmlib.c:1255,1290`) — das Desktop-Fenster gehört dem aufrufenden
  Prozess (pid 0). NewTOS' `init_desk_window` (`window.rs:503-507`) lässt
  `owner` auf dem `EMPTY`-Wert 0 und setzt keinen Besitzer. `wind_get(0,
  WF_OWNER)` liefert heute zufällig auf beiden Seiten 0; nach A56 Schritt 2
  wären „kein Besitzer" und „die Anwendung" ununterscheidbar — der
  `EMPTY`-Sentinel muss dann ein anderer Wert sein, und `init_desk_window`
  sollte den Aufrufer eintragen wie EmuTOS. Eigenes Thema.
- `pd.rs` Doc-Comment `:461-464` und `:584-585` erklären das Teilen des
  Deskriptors mit „pid 1"; nach Schritt 2 sind diese Sätze umzuschreiben.
