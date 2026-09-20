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
