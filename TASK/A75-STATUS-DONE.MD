# A75 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Abnahme offen

`appl_read` (11) blockiert jetzt auf leerer Queue, wie EmuTOS' `aqueue`; die
Rueckgabe ist 1 mit der Nachricht, ein 0-Rueckweg fuer „leer“ existiert nicht
mehr. `appl_write` (12) auf voller Queue bleibt bei 0 — die vom Lead in
`docs/COMPATIBILITY.md` sanktionierte Abweichung, nicht nachgebaut.
Bearbeitet 2026-09-20, Branch `desk-rebuild`, Basis HEAD `27e4328`,
**nicht committet**. Spec (vom Lead, uncommitted, unveraendert gelassen):
`docs/api/aes/application.md` „Blocking behaviour (A75, 2026-09-20)“,
`docs/COMPATIBILITY.md` „`appl_write` on a full application-message queue
returns 0“.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll, Lesen) | `aes/gemaplib.c:69-89` `ap_rdwr` → `ev_block` → `aes/gemqueue.c:85-125` `aqueue`: Bereitschaft `p_qindex > 0` (`:98-99`), bereit → `doq` + `azombie(e, 1)` (`:104-107`) = 1; leer → `evinsert(e, &p_qdq)` (`:120-124`) + `mwait`/`dsptch()` (`aes/gemasync.c:117-127`); geweckt durch einen spaeteren Schreibzugriff in dieselbe Queue (`:108-118`). `doq` kopiert `qpb_cnt` Bytes ungeprueft (`:76-81`) — die Bereitschaft ist „nicht leer“, nicht „`length` Bytes da“ |
| EmuTOS (Soll, Schreiben) | `aqueue` `:96-97`: `qpb_cnt <= QUEUE_SIZE - p_qindex` (128 Bytes = 8 × 16, `aes/struct.h:56`), sonst Block auf `p_qnq` — sicher nur, weil ein *anderer* geplanter Prozess die Zielqueue liest |
| NewTOS (vorher) | `tos-api/src/aes/appl.rs::appl_read`: ein `pop_into`, leer → 0; `appl_write`: `pd::post_words` → `MsgQueue::enqueue` gibt auf voller Queue `false` → 0 |
| NewTOS (nachher) | `appl_read` schlaeft auf `sched::wait_mesg(idx)` und prueft erneut (`appl.rs`), `crysbind` reicht das `Option<Switch>` (`mod.rs`); `appl_write` unveraendert, Kommentare auf den COMPATIBILITY-Eintrag ausgerichtet (`appl.rs`, `pd.rs`) |

## Was gebaut ist

* **`tos-api/src/aes/appl.rs`** — `appl_read(pb) -> Option<sched::Switch>`:
  Identitaet und Zielpuffer wie bisher aufgeloest (kein Deskriptor / `NULL`
  → 0, unveraendert), dann Schleife nach dem Muster von `evnt_mesag`:
  `pd::by_index(idx)` je Durchlauf, `pop_into` wenn eine Nachricht vorn
  steht (Kopie in den Puffer, `AP_MESAG`, 1), sonst
  `sched::sleep(sched::wait_mesg(idx))` — `Yield(Switch)` geht nach oben
  (S2-Neustart, nichts committet), `Recheck` prueft erneut. Doc-Kommentar
  mit den EmuTOS-Zeilen. `appl_write`: Doc-Absatz „Full queue: 0, not a
  block — a sanctioned deviation“ und der `post_words`-Kommentar zitieren
  den COMPATIBILITY-Eintrag statt AUD-P1-006 als „separate finding“.
* **`tos-api/src/aes/mod.rs`** — `crysbind`: `11 => return appl::appl_read(pb)`
  (wie 20–25).
* **`tos-api/src/aes/pd.rs`** (nur Kommentare) — `MsgQueue::enqueue`: der
  Volle-Queue-Absatz begruendet nach dem COMPATIBILITY-Eintrag (die alte
  Begruendung „kein AES-Scheduler“ ist ueberholt, das „in der Praxis
  unerreichbar“ begrenzt die Haeufigkeit, nicht die Abweichung);
  `wake_mesg_waiter` nennt `appl_read` als Warter.
* **Nicht angefasst:** AUD-P1-008 (`appl_read` setzt `AP_MESAG`) — die
  Spec-Sektion nennt es nicht. Kein Schreibseiten-Block. Keine Stubs.
* **`tos-api/tests/aes_appl_read_blocks.rs`** (neu; Harness nach
  `aes_pd_by_slot.rs`): ein Host, dessen `pump_input` — der Aufruf, den
  `sched::sleep` ohne lauffaehigen Peer macht — die Rolle des EmuTOS-Peers
  spielt: beim ersten Pump den Weckschluessel des laufenden Slots
  aufzeichnen und, falls geladen, per re-entrantem `appl_write(0, 16, msg)`
  in die Queue des Lesers schreiben (Re-Entrancy-Guard fuer den
  `dsptch()`-Pump des inneren Dispatches). Der Prozess-Slot ist fuer GEMDOS
  (`RUN`) *und* den Scheduler (`sched::set_running`) der laufende, damit
  der Weckschluessel beobachtbar ist; Drop setzt beides zurueck.
* **Drei bestehende Tests umgestellt** (`aes_pd_by_slot.rs`,
  `aes_app_return_descriptor.rs`, `aes_launch_boundary_reset.rs`): ihre
  `drain`-Helfer (`while appl_read().0 == 1`) und zwei „Queue ist leer“-
  Orakel bauten auf dem 0-bei-leer auf und wuerden jetzt endlos schlafen;
  jetzt Tiefenpruefung ueber `window::test_msg_count`/`test_pending_msg`
  (das Muster von `aes_window_content.rs::next_msg`). Die `appl_read(1)`-
  Pruefung (Screen Manager: keine lesbare Queue → 0) bleibt, sie
  blockiert nicht.
* **`docs/STATUS.md`** — Phase-13-Zeile (datierter Eintrag) + Zeile in
  „Recent Decisions“.
* **`measurements/measurement-2026-09-20-a75/`** — RUNINFO, Screendumps, Serial,
  Hosttest-Quittungen, Build-/Clippy-Logs, Methode (Skripte, Produkt-Diff,
  Test-Diff), SHA-256-Manifest. Keine `*.img`.

## Tests: rot vor, gruen nach

Gegenprobe in einer Scratch-Kopie der Crates mit `appl.rs`/`mod.rs`/`pd.rs`
= HEAD `27e4328` und der neuen Testdatei
(`measurements/measurement-2026-09-20-a75/host-tests/aes_appl_read_blocks-RED-before.txt`,
`…-GREEN-after.txt`):

| Test | vor | nach |
|---|---|---|
| (a) `appl_read_on_a_non_empty_queue_returns_1_with_the_message_without_sleeping` — ein Eintrag, `appl_read(0,16)` → `(1, probe)`, kein Weckschluessel gesetzt | gruen | gruen |
| (b) `appl_read_on_an_empty_queue_blocks_and_delivers_the_message_written_meanwhile` — leere Queue; beim ersten Pump `wait_cond == sched::wait_mesg(slot)`, der Peer schreibt, der fortgesetzte Aufruf liefert genau diese Nachricht, `pumps >= 2` | `appl_read on an empty queue returned 0 instead of blocking … left: 0 right: 1` | gruen |
| (c) `appl_write_on_a_full_queue_returns_0_without_blocking_sanctioned_deviation` — 8 × 16 Bytes → 1, der 9. → 0 ohne Schlaf, 8 in Reihenfolge zurueckgelesen | gruen | gruen |

Umgestellte Tests: `aes_pd_by_slot` 4/4, `aes_app_return_descriptor` 1/1,
`aes_launch_boundary_reset` 1/1 gruen
(`host-tests/converted-drain-tests-GREEN-after.txt`). Vor der Umstellung
lief `aes_pd_by_slot::reported_values_follow_emutos` in den
1 000 000-Pump-Waechter des `SilentHost` („a trap is spinning on an
exhausted pointer script“) — der inline-Drain der Elternqueue.

`cargo run -p xtask -- test` **693 → 696 / 0 / 2**. `cargo xtask build
--board rpi4` (tos-api + kernel neu uebersetzt) rustc tos-api 64 / kernel
94 — Delta 0. Clippy (`-p tos-api --tests`): lib 81 (A78-Basis 81, Delta
0); der einzige Treffer in einer beruehrten Datei, `items after a test
module` `pd.rs:346`, ist vorbestehend (HEAD: `mod init_tests :337` vor
`post_msg :383`); keiner in `appl.rs`, `mod.rs` oder den vier Testdateien.

## QEMU raspi4b (`measurements/measurement-2026-09-20-a75/`)

Injektor-Scratchkerne nach der A73-Methode (Scratch-Kopien, der Baum blieb
unberuehrt): vorher = HEAD-`appl.rs`/`mod.rs`/`pd.rs` (Kernel-Hash
`16eee915…` = A73s `after-run3`-Kern, reproduzierbar), nachher = Baum
(`1dac471f…`). Zwei Images (`tools/make-sd-image.sh`, 64 MiB): mit `#Z`-
TeraDesk-`NEWDESK.INF` (H02) und ohne (Platzhalter → Kernel-Desktop; mit
`#Z` startet die Shell TeraDesk direkt, ohne „`[desk] desktop up`“).
Sequenz A73-run3: HELLO.TXT markieren → File (88,8) → Info… (150,43) →
Klick → Dialog. **Alle 7 Dump-Paare bytegleich** (Kernel-Desktop
`dee57cca…`; TeraDesk hoch `bd5dff21…`, A `f7e77f96…`, B File-Tafel
`9e9d3c07…`, B2 `58c80998…`, C1 = D `7474f018…` — A/B sind die Hashes aus
A73), `grep -c "EXC\|PANIC\|panic"` = 0 in allen vier Serial-Logs.
Erwartet: TeraDesk, QED und cflib rufen `appl_read` nie (Spec „Caller survey
(A75)“).

## Nicht angefasst / offen

* **Pi-Abnahme offen** (Tester/Lead; nichts geflasht). Kein Bordpfad uebt
  ein blockierendes `appl_read` — kein ausgeliefertes Programm ruft es; der
  Blockbeleg ist Hosttest. `kernel/src/selftest_aes.rs` (Schreiben, dann
  Lesen auf nicht-leerer Queue) ist nicht in den Bootpfad verdrahtet und
  wurde nicht gefahren.
* `sched.rs`: der Doc-Kommentar zu `wait_mesg` nennt weiter nur
  `evnt_mesag` als Nutzer (Datei ausserhalb des Aenderungsumfangs, nicht
  angefasst).
* Der Slot-Modell-Rand (`pop_into` liefert bei `length` kuerzer als die
  Nachricht den Anfang und entfernt die Nachricht ganz; EmuTOS wuerde
  `p_qindex` unterlaufen) ist die vorbestehende, am Typ dokumentierte
  Abweichung — nicht Teil dieses Tickets.
* Ohne installierten Host (kein `pump_input`) schlaeft ein `appl_read` auf
  leerer Queue endlos — das ist der Block, nicht ein Fehler; auf dem Board
  ist immer ein Host installiert, Hosttests ohne Host duerfen `appl_read`
  nur auf nicht-leerer Queue rufen.
* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht, keine
  `*.img` behalten.
