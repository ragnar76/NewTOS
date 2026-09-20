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
