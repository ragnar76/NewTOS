# A112 — evnt_multi(MU_MESAG): bereits liegende vs. waehrend des Wartens eintreffende Nachricht (2026-10-17)

Ticket: `TASK/A112.md`. Phase 1: erst die Ursache, kein Produktcode geaendert.
Belege: `measurements/measurement-2026-10-17-a112/` (RUNINFO.TXT, serial/, method/).
Sonde: `sdk/examples/a112mesg.c` (+ Begleit-Accessory `sdk/examples/a112acc.c`).

## Urteil in zwei Teilen

1. **Die A25-Beobachtung war ein Sondenfehler.** Die fruehere Sondenfassung
   druckte `evnt_multi events=32 (MU_MESAG=1 MU_TIMER=32)`
   (`measurements/measurement-2026-10-17-a25/serial/attempt2-evnt_multi-observation/*.log`).
   Sie benutzte also `MU_MESAG = 1`. 1 ist `MU_KEYBD` (`tos-types/src/aes.rs:367`,
   EmuTOS `include/aesdefs.h:256`); `MU_MESAG` ist `0x0010` (`aes.rs:375`,
   `aesdefs.h:260`). Die Sonde wartete auf `MU_KEYBD|MU_TIMER`, „nur Timer“ war die
   richtige Antwort. Mit `0x0010` sieht `evnt_multi` die selbst gesendete Nachricht
   sofort (Fall A, und auch nach erneutem `appl_init`, Fall C), in memprot on und off.
   Die Quelle der fruehen Fassung ist nicht mehr vorhanden; die Logzeile selbst nennt
   die Konstante.

2. **Ein anderer, echter Defekt: `evnt_multi` schlaeft nur auf dem Eingabe-Weckschluessel.**
   Sobald waehrend des Wartens ein anderer AES-Prozess laeuft (z. B. ein Accessory),
   wecken weder eine an das Programm gepostete Nachricht noch dessen eigener
   `MU_TIMER` das Programm. Es bleibt haengen, bis Maus oder Tastatur etwas tun.
   Gemessen: Fall G2 (Nachricht von einem Accessory, 5000-ms-Timer) kommt bis zum
   Laufende (40 s nach Start) nicht zurueck; in der ersten Sondenfassung
   haengt schon der reine 500-ms-Timer (Fall E), solange das Accessory in seiner
   `evnt_timer`-Schleife laeuft.

## Messergebnis (QEMU raspi4b, Kern = A25-Kern, sha256 c582909a…)

| Fall | Ablauf | ohne Acc. | mit Acc. |
|---|---|---|---|
| E | leere Queue, `evnt_multi(MU_MESAG\|MU_TIMER, 500)` | 32 PASS | (Fassung 1: **haengt**) |
| A | `appl_write`(selbst) → `evnt_multi` | 16 + Bytes PASS | PASS |
| B | `appl_write`(selbst) → `evnt_mesag` | 1 + Bytes PASS | PASS |
| C | `appl_write` → `appl_init` erneut → `evnt_multi` | 16 PASS | PASS |
| D | zwei `appl_write` → zwei `evnt_multi` | FIFO PASS | PASS |
| F | eigenes `wind_open` (AES legt `WM_REDRAW` im eigenen Trap ab) → `evnt_multi` | 16, WM_REDRAW PASS | PASS |
| G | `evnt_multi(…, 5000)`, nichts kommt | 32 PASS | — |
| G1 | `evnt_mesag`; Accessory oeffnet/schliesst ein Fenster → `WM_REDRAW` an uns | — | 1, WM_REDRAW PASS |
| G2 | `evnt_multi(MU_MESAG\|MU_TIMER, 5000)`; Accessory wiederholt den Zyklus | — | **haengt** (weder Nachricht noch Timer) |

memprot on und off liefern dieselben Zeilen. G1 gegen G2 ist das Paar, das die
Ursache festnagelt: dieselbe Nachricht aus demselben anderen Prozess weckt
`evnt_mesag`, aber nicht `evnt_multi`.

## EmuTOS-Pfad

- Schnellpruefung beim Eintritt: `aes/gemevlib.c:220-228` — `if ((rlr->p_qindex > 0) || …) { ev_mesag(mebuff); what |= MU_MESAG; }`.
  Eine liegende Nachricht wird sofort geliefert (unser Fall A/C/D/F).
- Sonst wird **jedes** angeforderte Ereignis einzeln scharf gemacht und auf die Vereinigung gewartet:
  `gemevlib.c:231-264` — `iasync(MU_MESAG, &m)` (`:250-256`), `iasync(MU_TIMER, …)` (`:258-259`),
  `which = mwait(flags)` (`:261`), `acancel(flags)` (`:264`).
- `iasync` (`aes/gemasync.c:130-179`): `MU_MESAG` → `aqueue(FALSE,…)` parkt den Leser auf `p_qdq`
  (`aes/gemqueue.c:120-124`); `MU_TIMER` → `adelay` (`aes/geminput.c:496`) haengt ihn in die Delay-Liste.
- `mwait` (`gemasync.c:117-127`): `p_evwait = mask`; schlafen nur, wenn noch kein Bit der Maske gesetzt ist.
- Wecken aus **jeder** Quelle: Schreiber `aqueue(TRUE,…)` findet den geparkten Leser,
  `doq` + `azombie(e, 1)` (`gemqueue.c:108-118`); der Timer-Interrupt `tchange`
  (`aes/gemflag.c:32-60`) → `evremove` → `azombie`; `azombie` → `signal`
  (`gemasync.c:35-58,64-77`) setzt `p_evflg |= e_mask` und haengt den Prozess von `nrl`
  nach `drl`, wenn `p_evflg & p_evwait` — egal, welches der angeforderten Ereignisse es war.

## NewTOS-Pfad

- Schnellpruefung und Pruefung je Durchlauf sind vorhanden und korrekt:
  `tos-api/src/aes/event.rs:636-676` (Queue-Pop bei `MU_MESAG`, jeder Durchlauf, also auch beim Eintritt),
  `event.rs:677` (Timer-Deadline). Deshalb bestehen A–F.
- Blockieren: `event.rs:717` — `sched::sleep(sched::wait_input())`, **ein** Schluessel, Klasse `WAIT_INPUT`,
  unabhaengig von den Flags. Der Kommentar `event.rs:709-716` nennt die Vereinigung der
  Nachrichten-/Timer-Klassen ausdruecklich als „load-bearing at step 4“ — sie fehlt.
- `sched::sleep` (`tos-api/src/bdos/process/sched.rs:513-570`): mit lauffaehigem Peer wird der Aufrufer
  WAITING(`wait_cond = WAIT_INPUT`) und der Peer laeuft (`on_yield`, `sched.rs:417-424`).
  Ohne Peer: Leerlaufpass und sofortiges Re-Check („Ausgang (a)“) — das ist der Grund, warum es
  ohne Accessory funktioniert.
- Wecker: `pd::post_msg`/`post_words` → `wake_mesg_waiter` → `wake(wait_mesg(idx))`
  (`tos-api/src/aes/pd.rs:425-439,452-464,474-476`) — trifft nur `WAIT_MESG(idx)`.
  `idle_wake_pass` (`sched.rs:763-795`) weckt `WAIT_INPUT` nur bei Tastatur-/Zeigeraenderung
  (`:782-784`) und Deadlines nur fuer Schluessel der Klasse `WAIT_TIMER` (`:785-794`).
  Ein in `evnt_multi` WAITING(INPUT) stehender Prozess wird also von keinem der beiden erreicht.
- Zum Vergleich die korrekt schlafenden Geschwister: `evnt_mesag` `event.rs:842` (`wait_mesg(idx)`,
  darum besteht G1), `appl_read` `appl.rs:542` (`wait_mesg(idx)`, darum bestand die A25-Endfassung),
  `evnt_timer`/`wait_ms` `event.rs:888` (`wait_timer(deadline)`).

**Unterschied:** EmuTOS wartet auf die Vereinigung aller angeforderten Ereignisse (`mwait(flags)` +
`signal` aus jeder Quelle); NewTOS' `evnt_multi` wartet auf genau eine Klasse (`WAIT_INPUT`).
Die Pruefungen sind gleich, das Wecken nicht.

## Warum echte Programme heute laufen

- **Bereits liegende Nachrichten** — die meisten, die ein Programm bekommt — sieht `evnt_multi`
  beim Eintritt: `WM_REDRAW` aus eigenen `wind_*`-Aufrufen (Fall F), selbst geschickte Nachrichten
  (A), und die Nachrichten des Control Managers (Menue `MN_SELECTED`, Fensterrahmen `WM_*`): diese
  werden in derselben `evnt_multi`-Schleife inline erzeugt (`menu_intercept`/`window::intercept`,
  `event.rs:526-528`) und im selben Durchlauf ab `event.rs:636` gelesen.
- **Kein lauffaehiger Peer waehrend des Wartens.** Ohne Accessory, oder mit dem mitgelieferten
  `SYSACC.NAC`, das in `evnt_mesag` (`WAIT_MESG`) schlaeft, nimmt `sleep` den Leerlaufzweig und
  prueft jeden Durchlauf alles neu — Timer und Queue eingeschlossen.
- **Wo es schon heute greift (aus dem Code abgeleitet, nicht gemessen):** Waehlt man ein Accessory im Menue, postet die Schleife `AC_OPEN`,
  das Accessory wird lauffaehig, das Programm steht danach WAITING(INPUT). Es kommt erst wieder dran,
  wenn sich Maus oder Tastatur regen — im Accessory-Dialog bewegt der Benutzer die Maus ohnehin, daher
  faellt es nicht auf. `MU_TIMER`-Wartezeiten des Programms laufen in dieser Zeit nicht ab.
  Ein Accessory mit eigenem `evnt_timer`/`evnt_multi(MU_TIMER)` (Uhr, Monitor) wuerde ein
  wartendes Programm dagegen dauerhaft einfrieren, bis eine Eingabe kommt — genau Fall E/G2.
  Spiegelbildlich bekommt ein Accessory, das in `evnt_multi(MU_TIMER)` wartet, seinen Timer nur,
  wenn eine Eingabe es weckt (hier nicht eigens gemessen; dieselbe Zeile `event.rs:717`).

## Fix-Vorschlag (nicht umgesetzt)

EmuTOS `mwait(flags)`-Semantik nachbilden: `evnt_multi` schlaeft auf der **Vereinigung** der
Weckquellen, die seine Flags anfordern.

1. `sched`: neben `wait_cond` je Prozess eine Weckmenge — z. B. `wait_mesg_idx` (oder Bit) und
   `wait_deadline` (0 = keine) — und ein `sleep_multi(input: bool, mesg: Option<usize>, deadline: Option<u64>)`.
   `wake(wait_mesg(idx))` trifft auch Waiter, deren Menge `mesg == idx` enthaelt; `idle_wake_pass`
   weckt zusaetzlich jeden Waiter mit `wait_deadline != 0 && wait_deadline <= now`; beim Aufwachen
   werden alle Teile geloescht (EmuTOS `acancel`, `gemevlib.c:264`). Entspricht `signal` +
   `p_evwait`-Maske (`gemasync.c:35-58,117-127`).
2. `event.rs:717`: Menge aus den Flags bauen — Input, wenn `MU_KEYBD|MU_BUTTON|MU_M1|MU_M2`
   **oder** `MU_MESAG` (die Control-Manager-Nachrichten entstehen aus Eingabe), `mesg = idx` bei
   `MU_MESAG` (`iasync(MU_MESAG)`, `gemevlib.c:250-256`), `deadline` bei `MU_TIMER`
   (`iasync(MU_TIMER)`, `:258-259`, die schon persistierte Deadline aus `arm_or_get_deadline`).
3. Kein anderer Aufrufer muss sich aendern; `evnt_mesag`/`appl_read`/`evnt_timer` behalten ihre
   Einzelschluessel.

Alternative, kleiner aber unsauber: in `idle_wake_pass` alle `WAIT_INPUT`-Waiter auch auf Deadline
und Queue pruefen — vermischt Klassen und braeuchte Zugriff auf AES-Queues aus `sched`; nicht empfohlen.

## Test rot → gruen

- **QEMU (vorhanden, heute rot):** `method/run-all.sh`, Laeufe `acc-on`/`acc-off`: Fall G2 muss
  `G2 events=16`, `msg0=20`, `msg3=1`, `case G2(acc,evnt_multi) PASS` und `RESULT PASS` drucken.
  Heute fehlt alles nach „G2 entering“. Zusaetzlich sollte die Fix-Abnahme eine Variante mit Fall E
  auf der Accessory-Karte fahren (reiner Timer bei laufendem Peer, Fassung 1 der Sonde).
- **Host-Test (vorgeschlagen, nicht committet, weil er ohne Fix rot waere):** nach dem Muster von
  `tos-api/tests/aes_appl_read_blocks.rs` zwei Slots im `PROCESS_TABLE`, der zweite lauffaehig
  (Peer). Slot 1 ruft `evnt_multi(MU_MESAG|MU_TIMER, 1000)` ueber den echten LP64-AES-Dispatcher
  mit leerer Queue → `SleepOutcome::Yield` (Peer laeuft). Dann (a) `appl_write`/`post_words` an Slot 1
  → `PROCESS_TABLE[1].runnable` muss `true` sein (heute `false`); (b) ohne Nachricht die Host-Uhr
  ueber die Deadline stellen und einen Leerlaufpass aus Peer-Sicht ausloesen → `runnable == true`
  (heute `false`). Danach die wieder ausgefuehrte `evnt_multi` liefert 16 bzw. 32.

## Nebenbeobachtung (ausserhalb A112, unveraendert)

`appl_write` kann kein anderes AES-Programm adressieren: jeder Client meldet pid 0
(`appl.rs:218-221`), und `identity_of(0)` ist immer der Aufrufer (`appl.rs:235-237`, Kommentar
„no accessories“ — Accessories gibt es seit ACC02-04). In EmuTOS haben Accessories pid 2… und
`appl_write(0, …)` aus einem Accessory landet beim Programm. Deshalb ist die „Nachricht von einem
anderen Prozess“ in dieser Sonde ein vom AES gepostetes `WM_REDRAW`, kein `appl_write`.
