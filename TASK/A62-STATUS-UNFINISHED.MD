# A62 — Quittung

**Zustand: UNFINISHED** · 2026-09-20 · kein Commit (Untersuchung, kein Umbau)

**UNFINISHED heißt hier: die Untersuchung ist fertig, der Umbau steht aus.**
Beide Klärungspunkte des Tickets sind mit Messung beantwortet; die
Entscheidung über den Umbau liegt beim Anwender (Bericht, Abschnitt 6).

Bericht: [docs/analysis/aes-process-table-and-apid-2026-09-20.md](../docs/analysis/aes-process-table-and-apid-2026-09-20.md).
Artefakte: [measurement-2026-09-20-a62/](../measurement-2026-09-20-a62/RUNINFO.TXT).
Basis `desk-rebuild` @ `8f052b2`, gepinnte EmuTOS-Quelle `1a7484ab…`.

## 1. Punkt 1 — ab wann bricht es? Gemessen: **Verschachtelungstiefe 9, nicht acht Programme nacheinander**

- **Neun verschiedene Programme nacheinander** (`SEQ1.NAP` … `SEQ9.NAP`, eine
  Binärdatei unter neun Namen, `#Z`-Autorun + `shel_write(SHW_EXEC)`-Kette,
  QEMU raspi4b, `serial/seq-serial.log`): `appl_init` = **1 bei allen neun**,
  nie −1. Alle landen in GEMDOS-Slot 0, pid 1; `pd::register` benennt denselben
  Deskriptor um (`appl_find(vorgänger) = -1`). Die Ticket-Prämisse „nicht
  gebunden, wenn mehrere verschiedene Programme nacheinander laufen" ist
  **widerlegt**: der Schlüssel ist pid = Slot + 1, nicht der Name.
- **Verschachtelt** (`NESTTST.NAP` `Pexec`t sich selbst, 16 tief — der
  TeraDesk-Startweg nach H05, `serial/nest-serial.log`): Tiefe 1–8 →
  `appl_init` 1…8; **Tiefe 9 (Slot 8, pid 9) → −1**, `appl_write(self)` → 0;
  Tiefe 16: `Pexec` des 17. → −39 (`ENSMEM`). Danach kehren alle 16 regulär
  zurück, `[desk] desktop up`.
- Dieselben Zahlen auf dem Host über den echten LP64-Dispatcher
  (`host/host-probe.log`); zusätzlich: `register_process` bei voller Tabelle
  verliert den Namen still (`appl_find` = −1).

## 2. Punkt 2 — `appl_init` bei voller Tabelle

Heute noch **−1** (`tos-api/src/aes/appl.rs:221-225`, gemessen Tiefe 9–16).
EmuTOS `ap_init` (`aes/gemaplib.c:54-63`) kann nicht scheitern. Das Programm
läuft weiter, ist aber taub: kein Deskriptor → jede `WM_*`/`MN_SELECTED` wird
verworfen, `evnt_multi(MU_MESAG)` feuert nie (`event.rs:597-616`). cflib
(QED) prüft den Rückgabewert nicht (`cflib-21/appinit.c:40`); TeraDesk bricht
ab (`main.c:1821`).

## 3. Der Vorschlag „über den Slot indizieren", gegen den heutigen Code

Vollständig im Bericht, Abschnitt 4: was an der Tabelle hängt (Schlange, Name,
Flags, Wartekey `sched::wait_mesg(pid)`, Ressourcenschlüssel `resource.rs`
A39, `Window.owner`), der Umbau in sieben Schritten, was bricht (der
`pd.rs`-Unittest mit negativen pids, die A25/A28/A29-Tests über die Prämisse
„Slot 0 teilt den Desktop-Deskriptor", die `resource.rs`-`Proc`-Fixtures),
Aufwand als Liste. Kern: **Identität (Slot) und berichteter Wert (pid)
trennen** — dann ist die Tabelle per Bauart `MAX_PROCESSES + 1` groß.

Offene Entscheidung dabei: bekommt der Kernel-Desktop einen eigenen Index
(empfohlen; bereitet die EL0-Verlegung vor, macht A29s Test strenger) oder
teilt er weiter Slot 0.

## 4. Was nicht gefahren wurde

Ein reales GEM-Programm in Tiefe 9 (Taubheit hergeleitet aus `appl_write = 0`
und `event.rs`, nicht mit QED gezeigt). Kein Screendump nach der 16er-Kette
(nur seriell `desktop up`).

## 5. Aufräumen

Kein Produktcode geändert. Sonden (`nesttst.c`, `seqtst.c`, Host-Test
`zz_a62_probe.rs`) liefen ausschließlich in einer Scratch-Kopie des Baums;
Kopien unter `measurement-2026-09-20-a62/method|host`. `git status --short`
vor der Arbeit leer, danach nur die neuen Dateien (Bericht, diese Quittung,
A56-Quittung, Messverzeichnis); `kernel/src/host_env.rs` sha256
`287ff7415805b9cf0b7efe74487a5f24dd06f121ddd61cdb5f9e2d8a15618e3e` vorher =
nachher. Keine Hardware-/Outward-Aktion, kein Commit, kein `git
checkout/clean/stash`.

## Spec (Researcher, 2026-09-20)

**Auftrag:** Spec für A62 (Identität über den Slot) + A56 (berichtete
pid-Werte nach EmuTOS) in `docs/api/aes/`, auf Basis dieses Berichts (§3
Tabelle, §4 Umbau, §6 Vorschlag). Kein Produktcode, kein Commit. Quellen
frisch gegen `references/emutos` @ `1a7484ab` gelesen (nicht nur aus diesem
Bericht übernommen): `aes/gempd.c`, `aes/geminit.c:236-257,850-888`,
`aes/gemaplib.c:54-63,251-261`, `aes/gemshlib.c:572,592-689`,
`aes/gemwmlib.c:851-863,904,1255,1290,1377,1579-1580`, `aes/gemctrl.c:81-368`,
`aes/gemevlib.c:100-117`, `aes/struct.h:36`, `include/sysconf.h:37`,
`include/aesdefs.h:509` (`WM_OX = int_out[1]`, bestätigt `WF_OWNER`s
Zielindex).

**Geschrieben:** eine neue Sektion "AES process identity and pid" in
[`docs/api/aes/application.md`](../docs/api/aes/application.md) (EmuTOS-Modell
+ NewTOS-Vertrag in zwei Stufen + Folgen für A39/A28/H02/H05); die drei
benannten Korrekturen dort (`:452-453`-Äquivalent, `:194-197`-Äquivalent,
`:587`-Äquivalent — Zeilennummern haben sich durch die Einfügung verschoben,
Textanker sind unverändert auffindbar); eine vierte, bislang unbenannte
Dopplung derselben falschen Aussage in
[`docs/api/aes/_state.md`](../docs/api/aes/_state.md) (§1-Bullet `totpds`, §2
"Application id", Kopf-Callout, §7-Schlussbullet — vier Stellen, eine
Quelle laut MEMORY "Eine Begründung steht selten nur einmal"), plus die
`rlr`- und `ctl_pd`-Einträge dort um den Pexec-Nichtneuzuweisungs- bzw.
`msg[1]`-Stempel-Hinweis ergänzt. [`docs/api/aes/event.md`](../docs/api/aes/event.md):
`evnt_mesag`s `WM_ARROWED`-Fastpath-Absatz und Edge-Case #7 um den Wert
(0 = Leser, 1 = normale Warteschlange) ergänzt, die 16-Byte-Nachricht-Bullet
präzisiert. [`docs/api/aes/window-graphics.md`](../docs/api/aes/window-graphics.md):
`WF_OWNER`-Fußnote + neuer Callout unter der `wind_get`-Tabelle (Wert = 0,
nicht die gespeicherte Identität), `wind_create`s `w_owner`-Zeile,
neuer Callout unter der `WM_*`-Nachrichtentabelle (welcher Absender welchen
Wert stempelt, mit der `WM_ARROWED`-Ausnahme), neue "NewTOS implementation
notes"-Zeile. Keine Datei außerhalb `docs/api/aes/` und `TASK/` berührt;
`shell-loop.md`/`scrap-shell.md` bewusst **nicht** verändert — sie hatten das
`ap_id == 0`-Modell für Desktop/erstes Programm bereits seit 2026-09-01
korrekt (siehe unten), die neue Sektion erweitert das nur um die verschachtelte
`Pexec`-Kette und die `msg[1]`/`WF_OWNER`-Hälfte.

**Vertragssätze, wörtlich** (aus `application.md` §"AES process identity and
pid", NewTOS-Vertrag (b)):

> **0** for every member of the application chain — the kernel-context
> desktop, the program it launches, and any `Pexec` child at any nesting
> depth.

> **1** only as a **sender stamp** on `msg[1]` … there is **no NewTOS
> process** behind this value; it is a constant the poster writes.

> **0** for `WM_REDRAW`: … `rlr` is the application (pid 0) at send time, not
> the Screen Manager.

> **0**, exceptionally, for a **non-queued** `WM_ARROWED` … the target's own
> later `evnt_mesag` call then synthesizes the message itself … with
> `msg[1] = rlr->p_pid` — the **reader's own** pid, 0.

> **`appl_write`/`appl_read` target 0** resolves to **the currently-scheduled
> client** (the innermost active chain member).

> **`WF_OWNER`** (`wind_get`, `int_out[1]`): **0** for every application
> window.

Und Vertrag (a), Identität:

> A NewTOS AES client's *identity* … is keyed by its **GEMDOS process slot**,
> plus one reserved index for the kernel-context desktop (table size
> `MAX_PROCESSES + 1`). … **`appl_init` never returns `-1`** for "table
> full."

**Die drei Korrekturen, vorher/nachher** (Zitate aus dem Stand vor dieser
Änderung, siehe git-Historie von `application.md` für den genauen Diff):

1. *Vorher* (Edge case 1): "The AES pid is small (0/1 = system processes, 2+
   = accessories/applications, `geminit.c:875-877`)." *Nachher:* pid 0 = die
   ganze Anwendungskette (kein "System-Prozess"), pid 1 = Screen Manager
   (ein echter zweiter Prozess), pid 2+ = nur Accessories.
2. *Vorher* (unter `appl_init`): "The documented `-1` failure return is the
   contract for a NewTOS that *can* fail to allocate an application slot."
   *Nachher:* das lizenzierte die Abweichung statt sie zu benennen; EmuTOS'
   Vertrag ist "nie −1", ohne Ausnahme — die heutige `-1` bei Tiefe 9 ist der
   von A62 benannte Defekt, keine erlaubte Variante.
3. *Vorher* (NewTOS implementation notes): "`ap_id` = the Phase-06 process
   id — the AES application is a process; `appl_init`/`appl_exit` bracket
   its life exactly as `Pexec`/`Pterm` do." *Nachher:* `ap_id` ist **nicht**
   die GEMDOS-Prozess-id (das wäre Slot+1); EmuTOS berichtet 0 für die ganze
   Kette. Nur die *Lebensdauer*-Klammerung (appl_init/appl_exit ↔
   Pexec/Pterm) war richtig gemeint, nicht die Zahl.

Vierte (im Auftrag nicht namentlich genannte, aber dieselbe Aussage
duplizierende) Korrektur: `_state.md` §1 (`totpds`-Bullet) und §2
("Application id") trugen wortgleich dieselbe falsche 0/1-System-Aussage wie
`application.md`; beide korrigiert, plus der Kopf-Callout und der
§7-Schlussbullet, die es referenzierten.

**Cited vs. inferred:** jede Zahl/jedes Verhalten im EmuTOS-Modell (a) und in
den Vertragssätzen zu 1/0-Werten ist an eine `file:line`-Stelle gebunden, die
in dieser Sitzung gelesen wurde (siehe Quellenliste oben; `ap_sendmsg`,
`ct_msgup`/`handle_arrow_msg`-Aufrufstellen, `ev_mesag`s Fastpath, `w_redraw`s
Aufrufkette über `draw_change`, `WM_OX = int_out[1]` aus `aesdefs.h:509` alle
frisch nachgelesen, nicht nur aus dem Auditor-Bericht übernommen). Die
Abschnitte "Consequences for A39/A28/H02/H05" sind **Schlussfolgerung, nicht
neu verifizierte EmuTOS-Fakten** — sie übernehmen den bereits vom Auditor
belegten NewTOS-seitigen Sachverhalt (`resource.rs`/A39, H05-Regression) und
ziehen daraus, was für die Spec folgt; als solche im Text markiert
("reasoned, not re-derived here").

**Offene Entscheidungen (nur echte, an den Anwender/Developer):**

1. **Desktop-Identität** — eigener Tabellenindex (empfohlen im Bericht §6)
   oder weiterhin Slot 0 teilen. Die Spec dokumentiert beide Male nur den
   **berichteten Wert** (0 für den Desktop als Teil der Kette); welchen
   internen Index der Kernel-Desktop bekommt, ist Umbau-Entscheidung, nicht
   Spec-Entscheidung — unverändert offen, wie im Bericht §6 benannt.
2. **`appl_write`/`appl_read`-Zielauflösung für Wert 0** ("der aktuell
   laufende Klient") ist als Vertrag benannt, aber ihre Implementierung hängt
   von der noch zu bauenden Slot-Identität ab (A62 Stufe 1) — die Spec
   beschreibt das Soll, nicht den Mechanismus.

**Diff-Umfang:** 4 Dateien geändert (`application.md`, `_state.md`,
`event.md`, `window-graphics.md`), alle unter `docs/api/aes/`; keine Datei
gelöscht oder umbenannt; keine Zeilennummern-Referenzen in anderen Specs
gebrochen (Suche nach den korrigierten Wortlauten in allen vier Dateien
bestätigt: nur noch innerhalb der Korrektur-Zitate selbst, keine stehen
gebliebene falsche Standalone-Aussage). `shell-loop.md`/`scrap-shell.md`
nicht verändert (bereits korrekt, s.o.). Kein Produktcode, kein Commit.

## Umbau Stufe 1 (Developer, 2026-09-20) — Identität über den Slot, `appl_init` nie −1

**Zustand nach dieser Arbeit: Stufe 1 implementiert, Hostbelege + QEMU-Belege
vorhanden; UNFINISHED bleibt, weil die Abnahme auf echter Pi 4 (Tester/Lead)
und Stufe 2 (A56, berichtete Werte nach EmuTOS) ausstehen.** Kein Commit, kein
`git checkout/restore/clean/stash`, keine Hardware-/Outward-Aktion. Basis
`desk-rebuild` @ `86b871d`, Baum vorher sauber. Artefakte:
[`measurement-2026-09-20-a62/fix/`](../measurement-2026-09-20-a62/fix/).

### 1. Was gebaut wurde (Spec `docs/api/aes/application.md` § "AES process identity and pid", Vertrag (a))

| Datei | Änderung |
|---|---|
| `tos-api/src/aes/pd.rs` | Tabelle `NUM_PDS = MAX_PROCESSES + 1` (17), Index = **Identität**: GEMDOS-Slot 0..15, `DESKTOP = 16` für den Kernel-Desktop (eigener Index, Entscheidung des Leads). `Aespd` ohne `in_use`/`pid`. `open(idx) -> &mut Aespd` **kann nicht scheitern** (EmuTOS `ap_init`, `gemaplib.c:54-63`); `by_index`, `find_by_name -> Option<usize>` (EmuTOS `fpdnm`, `gempd.c:38-57`: Walk ohne Lebendprüfung, ein Name je Slot), `close`/`close_if_open`/`register` per Index; **neu `release(idx)`**: Flags = 0 + Queue leeren (die Vereinigung von `gemshlib.c:636` und `:650-657`/`ap_exit`). `post_msg`/`post_words` per Index, Trace bekommt weiter den berichteten Wert. Unit-Test auf die Slot-Prämisse umgeschrieben (der alte „Tabelle voll"-Fall existiert nicht mehr). |
| `tos-api/src/aes/appl.rs` | `current_identity()` (Slot bzw. `DESKTOP` bei `RUN == null`), `reported_pid(idx)` (**Stufe 1 = heutige Werte**: Slot + 1, Desktop 1), `identity_of(pid)` (Umkehrung für `appl_write`/`appl_read`/Testhaken; Wert 1 löst vom Aufrufer aus: Desktop → Desktop, Prozess → Slot 0), `current_pid()` = `reported_pid(current_identity())`. `appl_init` ohne −1-Pfad; `appl_find` → `reported_pid` des gefundenen Index. |
| `tos-api/src/aes/mod.rs` | `close_client_if_open(slot: usize)`; **neu** `release_client(slot)`; `register_process(slot, …)` direkt per Slot; `pid_of_slot` bleibt als Wert-Übersetzung. |
| `tos-api/src/aes/event.rs` | `evnt_multi`/`evnt_mesag`/`menu_intercept` auf Identität; `msg[1]` weiter `current_pid()` (Wert unverändert). |
| `tos-api/src/aes/window.rs` | `Window.owner` = Identität, `NO_OWNER = -1` (Slot 0 ist eine gültige Identität; das Desktop-Fenster meldet `WF_OWNER` weiter 0); `owner_of(handle)`, `send_redraw`/`intercept`/`post_untopped`/`post_to_owner` per Identität; `WF_OWNER` → `reported_pid`; `wind_create`/`WF_NEWDESK` speichern `current_identity()`; Testhaken übersetzen den berichteten Wert per `identity_of`. |
| `tos-api/src/aes/resource.rs` | `RscEntry.owner: usize` (`NO_OWNER = usize::MAX`) statt `pid: i16` (0 = frei kollidierte mit Slot 0); `caller_identity()`; `on_process_exit(slot)`; `selected`/`register`/`load_into_user` per Identität; Unit-Test-Fixture `Proc` nachgezogen. A39 in der Sache unverändert. |
| `tos-api/src/bdos/process/sched.rs` | `wait_mesg(idx: usize)`. |
| `tos-api/src/bdos/process/pterm.rs` | `on_process_exit(slot_idx)`; **`release_slot` ruft `aes::release_client(slot_idx)`** — deckt jeden Endepfad (`Pterm`/`Ptermres`/`Pterm0`/Ctrl-C/Fault) und damit den `handle_pexec`-Nebenbefund (§8 des Berichts): ein per EL0-`Pexec` gestartetes Kind, das mit `AP_OPEN` und ungelesener Nachricht stirbt, hinterlässt dem nächsten Mieter des Slots nichts mehr. `exceptions.rs` unverändert (der Reset sitzt im Prozessmodell, nicht im Handler). |
| `kernel/src/launch.rs` | `close_client_if_open(child.slot)` (bleibt als `sh_ldapp`-Schritt; nach dem Slot-Release strukturell No-op). |
| `kernel/src/shell_loop.rs` | Lückenlisten-Punkt 10 umformuliert: Identität erledigt (A62), offen bleibt nur der berichtete Wert (A56). |
| `tos-api/tests/aes_pd_by_slot.rs` | **neu**, vier Tests (a)–(d), s. u. |
| `tos-api/tests/aes_app_return_descriptor.rs`, `aes_launch_boundary_reset.rs` | nur der Helfer `launcher_cleanup` auf die Slot-Signatur + ein Absatz im Kopfkommentar zur geänderten Prämisse (Spec-Beleg: Vertrag (a)). **Alle Assertions unverändert, beide grün.** Ehrlich benannt: beide Tests lassen ihr „Kind" ohne `RUN` laufen, also auf dem Desktop-Deskriptor; die Slot-keyed Variante ist `aes_pd_by_slot.rs` (c). |

**Nicht geändert (Stufe 2):** kein berichteter Wert. `ap_id`, `appl_find`,
`WF_OWNER`, `msg[1]`, `appl_write`-Ziel liefern dieselben Zahlen wie vorher
(Test (d) pinnt sie; NESTTST-Sonde Tiefe 1–16 bestätigt es maschinell).

**Eine Auslegung, die der Lead prüfen sollte:** der Auftrag sagt für Test (a)
und die NESTTST-Sonde „→ alle 1" bzw. „Tiefe 9 → jetzt `appl_init` = 1". Das
widerspricht der Regressionsbedingung „kein berichteter Wert ändert sich"
(Tiefe 2 meldet heute 2, gemessen). Ich habe die Regressionsbedingung
umgesetzt: Tiefe *k* → *k* (Slot + 1), nie −1. „Alle 1" wäre Stufe 2.

### 2. Tests — rot vor, grün nach

`tos-api/tests/aes_pd_by_slot.rs` gegen den unveränderten Baum
(`host-tests/red-before-aes_pd_by_slot.txt`; für den Lauf war die eine
Zeile `close_client_if_open(slot)` von Hand auf die alte Signatur gesetzt
und danach zurückgeschrieben):

    every_nested_process_gets_a_descriptor_and_appl_init_never_returns_minus_one ... FAILED
      depth 9: appl_init returned -1
    nine_programs_in_one_slot_rename_one_descriptor ... ok            (Regressions-Pin)
    releasing_a_slot_resets_its_descriptor_and_queue ... FAILED
      left: (1, 2989)  right: (0, 0)   -- der neue Mieter las die Nachricht 0x0BAD des toten
    reported_values_are_unchanged_by_the_identity_rework ... ok       (Regressions-Pin)

Nach dem Umbau (`host-tests/green-after-aes_pd_by_slot.txt`): 4/4. Die
verwandten Tests `aes_appl_init` (A25), `aes_app_return_descriptor` (A29),
`aes_launch_boundary_reset` (A28), `aes_rsrc_per_process` (A39, 5/5),
`aes_window_content`, `aes_window_move_blit` (7/7), `aes_global_layout`:
alle grün (`host-tests/green-after-related-*.txt`).

    cargo run -p xtask -- test   vorher 666 / 0 / 1   (selbst nachgemessen)
                                 nachher 670 / 0 / 1   (+4 = die neue Datei)

### 3. Bauten, Warnungen, Clippy

    xtask build --board qemu-virt   exit 0, Compiling tos-api + kernel, rustc 64 / 94
    xtask build --board rpi4        exit 0, Compiling tos-api + kernel, rustc 64 / 94
    clippy --features board-qemu-virt   tos-api 81 / kernel 125
    clippy --features board-rpi4        tos-api 81 / kernel 125
    Baseline 64/94, 81/125 — Delta 0. ELF-Hashes je Board verschieden (echte Bauten).
    Keine Clippy-Meldung zeigt auf pd.rs/appl.rs/event.rs/sched.rs/pterm.rs/launch.rs/
    shell_loop.rs; die Treffer in window.rs/resource.rs sind die vorbestehenden
    `Aespb`-Sichtbarkeitswarnungen.

### 4. QEMU raspi4b (Produkt-Kernel `--release --board rpi4`, `41f9f82c…`, KEINE Instrumentierung)

Karten aus `tools/make-sd-image.sh` (Hauptbaum, `5d51048c…`) + INF-Dateien der
A41/A62-Methoden (Hashes `fix/build/kernel-and-image-hashes.txt`). Alle Läufe:
`grep -c "EXC\|panic"` = 0.

- **NESTTST-Sonde** (`serial/nest-serial.log`, Karte + Sonde der Auditor-Messung):
  Tiefe 1–16 → `appl_init` = 1…16, `appl_write(self)` = 1 in **jeder** Tiefe,
  `WF_OWNER` = Tiefe (1–15), `appl_find(NESTTST)` = 1; Tiefe 16: `wind_create` = −1
  (Fenstertabelle, eigenes Thema, wie vor dem Fix), 17. `Pexec` → −39; alle 16
  kehren regulär zurück, `[desk] desktop up (1 drives)`. Vorher: Tiefe 9–16 → −1,
  `appl_write(self)` = 0.
- **Smoke, bytegleich** zu `measurement-2026-09-19-a41/part2/screendumps/`:
  `smoke-desk-up.png` 0 Pixel, `smoke-qed-up.png` 0 Pixel,
  `smoke-tera-up.png` 0 Pixel gegen `product1-tera-colour-icons-idle.png`.
- **H05-Sequenz** (Scratch-Kopie mit dem A41/A39-Injektor, `full1.actions` aus
  `measurement-2026-09-19-a39/method/`, Kernel `92e0947c…`): TeraDesk → HARD DISC
  C → APPS → QED → QED.NAP doppelklicken (Kind in Slot 1) → QED „Datei / Beenden"
  → Rückkehr. `serial/full1-serial.log`: `[exec] preparing program (slot 1)`,
  kein EXC. Screendumps `full1-D-before-start.png` gegen
  `full1-K-after-return-10s.png`: **134 Pixel Differenz, alle im Rechteck
  (47,207)–(109,282) = der Mauszeiger**; Infozeile „610370 bytes used in 2 items."
  und Icons identisch; H (0 s) = I (2 s) = K (10 s) bitgleich (0 Pixel). Das ist
  der A39-Beleg (TeraDesks Ressourcen überleben QEDs `rsrc_load`/`rsrc_free`)
  unter der neuen Identitätsschlüsselung. `full1-F-qed-up.png` zeigt QEDs
  Menüleiste (mit „Datei" hervorgehoben, weil der Screendump 1 s nach der Marke
  fällt und der Zeiger da schon auf dem Titel steht — Harness-Timing, kein Befund).
- **quit2-Sequenz** (A41 Teil 2, TeraDesk beenden → Kernel-Desktop kommt zurück,
  Kernel `2ab40394…`): `quit2-A-idle`, `-B-window-open`, `-D-quit-dialog` je
  **0 Pixel** gegen `inj4-*`; `quit2-G-desktop-up-after-quit.png` 2606 Pixel im
  Rechteck (305,89)–(399,420) = ausschließlich die Datum/Uhrzeit-Spalte des
  Fensters (Karte heute erzeugt: `09-20-26 10:44` statt `09-19-26 20:19`), sonst
  identisch; `'DESKTOP.NAP' exited with rc=0` → `[desk] desktop up (1 drives)`.
  Damit ist die Grenze belegt, die durch diesen Umbau **zwei** Identitäten hat
  (Slot 0 = TeraDesk, 16 = Desktop): der Desktop empfängt nach der Rückkehr und
  malt sein Fenster.

### 5. Nebenbeobachtungen (nicht angefasst)

- `kernel/src/desk/mod.rs:180-186,646`: Doc-Kommentare nennen noch
  `pd::open(current_pid())`/`pd::by_pid(1)`; die Funktionen heißen jetzt
  `open(idx)`/`by_index`. Nur Kommentare, außerhalb des Scopes.
- `kernel/src/selftest_sleepwake_switch.rs:345`: Kommentar nennt `by_pid`
  (unverdrahteter Selftest, baut).
- `docs/STATUS.md` nicht angefasst (Lead-Scope; beim Commit nachziehen).
- Blank-Namens-Suche (`appl_find` mit acht Leerzeichen) findet jetzt Index 0 →
  1, wie EmuTOS' blank initialisierte `p_name` (`geminit.c:868`); vorher fand
  sie den ersten `in_use`-Eintrag ohne Namen = ebenfalls 1 nach dem
  Desktop-`appl_init`. Kein realer Aufrufer.

### 6. Offen

- Abnahme auf echter Pi 4 (Doppelklick-Pfad, H05 interaktiv) — Tester/Lead.
- Stufe 2 (A56): `reported_pid` auf EmuTOS' Werte, `msg[1]`-Stempel an den
  Control-Manager-Postern, `identity_of(0)` → aktueller Klient.

## Umbau Stufe 2 (Developer, 2026-09-20) — berichtete Werte nach EmuTOS (= A56)

**Zustand nach dieser Arbeit: Stufe 2 implementiert, Host- und QEMU-Belege
vorhanden; die Abnahme auf echter Pi 4 (Tester/Lead) bleibt offen.** Kein
Commit, keine Hardware-/Outward-Aktion. Basis `desk-rebuild` @ `25ff70c`
(Stufe 1). Quittung: [A56-STATUS-DONE.MD](A56-STATUS-DONE.MD) (ersetzt
`A56-STATUS-UNFINISHED.MD`); Artefakte
[`measurement-2026-09-20-a56/`](../measurement-2026-09-20-a56/RUNINFO.TXT).

Die zwei Übersetzungsstellen aus Stufe 1 tragen jetzt EmuTOS' Zahlen
(Spec § "AES process identity and pid", Vertrag (b)):

- `appl::reported_pid(idx)` → **0** für jede Identität (Slots 0–15, `DESKTOP`);
  `appl::identity_of(0)` → der aktuell laufende Klient (`current_identity()`),
  1/2+ → `None`. `appl_find`: Kettenname → 0, `"SCRENMGR"` → 1, sonst −1.
  `appl_write(1)` → 1 (angenommen, verworfen — Screen-Manager-Queue);
  `appl_write(2+)`/`appl_read(1|2+)` → 0.
- Poster: `menu_intercept` (`MN_SELECTED`), `window::intercept` (alle
  `WM_*`-Frame-Nachrichten inkl. `HOTCLOSE`-`WM_CLOSED` und gequeutes
  `WM_ARROWED`), `post_untopped` (`WM_UNTOPPED`) stempeln `msg[1]` =
  `appl::SCRENMGR_PID` (1); `send_redraw` (`WM_REDRAW`) bleibt beim Aufrufer
  (`current_pid()` = 0). `WF_OWNER` → 0. `pid_of_slot` → 0.
- Tests: `aes_pd_by_slot.rs` (d) umgeschrieben (4 rot vor, 4 grün nach), je
  eine `msg[1] == 1`-Assertion in `aes_window_drag.rs`/`aes_menu_click_ownership.rs`;
  A28/A29/A39-Tests von `desk_pid > 0`/`assert_ne!(pid, pid)` auf 0 bzw. Slot.
  Referenz-Harness: `global[2]` wird verglichen (`GLOBAL_EXCLUDED` [11, 12]),
  `Expected::ApId` entfernt, r2 244/247 `int_out[1]` → `Value(0)`. 204/204.
  `cargo run -p xtask -- test` 670 / 0 / 1; beide Boards 64/94, Clippy 81/125, Delta 0.
- QEMU: NESTTST Tiefe 1–16 `appl_init` = 0, `WF_OWNER` = 0, `appl_write(self)` = 1;
  Smoke, H05 (full1), quit2, A78 bytegleich zu Stufe 1 / A78 (bis auf die
  Uhrzeitspalte in quit2-G); H02 `MN_SELECTED msg[1]` = 1 auf der seriellen Spur.

Offen aus Stufe 2: `appl_write(1, WM_REDRAW)` → `w_redraw_desktop` (EmuTOS
`hctl_msg`, `gemctrl.c:380-384`) hat keinen Spec-Eintrag und ist nicht gebaut
(Researcher). Der Auslegungspunkt aus Stufe 1 („alle 1" vs. „k") ist damit
gegenstandslos: alle 0.
