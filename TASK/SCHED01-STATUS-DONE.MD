# SCHED01 — Abnahmebericht

Status: **UMGESETZT, Abnahmekriterium erfuellt** (QEMU `raspi4b` + `virt`,
headless)
Beleg: `measurement-2026-09-29-sched01/`
Basis: `desk-rebuild`, HEAD `3460eef`, Aenderung **unverbaut im Baum**
(nicht committet). Nichts geflasht, nichts ausgeworfen, keine Pi gefahren,
`/Volumes/BOOT` nicht beruehrt, kein `git checkout/restore/clean/stash/reset`.

---

## 1. Die Abnahme

> Die Ball-Demo beendet sich **mit** geladenen Accessories; der Desktop kommt
> vollstaendig zurueck und meldet seine Menuezeile an. Dazu TeraDesk beenden,
> ebenfalls mit Accessories. Regressionsflaeche: ACC02–ACC06 und jeder
> Programmstart.

Erfuellt, alles vier gemessen. Der entscheidende Ausschnitt
(`measurement-2026-09-29-sched01/raw/ball-exit-trace.log`):

```
267 >> AES op=19 appl_exit
    runset cur=2 : s0(run=0 …) s1(run=0 …) s2(st=2 run=0 ctx=0 cond=0x0)
[sched] switch -> slot 0     (Accessory 1 holt AC_CLOSE, blockiert wieder)
[sched] switch -> slot 1     (Accessory 2 desgleichen)
[sched] switch -> slot 2     <-- kommt zurueck
272 >> AES op=19 appl_exit   (der S2-Neustart)
    runset cur=2 : s0(run=0 …) s1(run=0 …) s2(st=2 run=1 ctx=1 cond=0x0)
    << -> 0
[shell] sh_ldapp: 'C:\BALL.NAP' exited with rc=0
314 >> AES op=30 menu_bar in=[1,…]
[desk] desktop up (1 drives, 0 window(s) from the context)
364 >> AES op=25 evnt_multi
```

**Das eine Bit, das PORT08 als Unterschied gemessen hat, wechselt jetzt im
`dsptch` des Aufrufers selbst von 0 auf 1** — zwischen Eintritt (267) und
Neustart (272). Vorher endete das Protokoll an derselben Stelle nach
`-> slot 1` und es kam kein einziger `svc` mehr.

Dreimal unabhaengig gefahren, identischer Ablauf.

---

## 2. Die Entscheidung: `dsptch` allein, **nicht** der Programmstart

Der Auftrag liess beides offen und verlangte, erst zu lesen. Gelesen, und die
Quelle entscheidet es:

**EmuTOS hat fuer ein gestartetes Programm gar keinen Aufnahmeschritt, weil es
keinen braucht.** `sh_ldapp` startet ein Programm **unter der AESPD der
Shell**: es benennt `rlr` um und ruft `dos_exec`
(`references/emutos/aes/gemshlib.c:604-605` fuer den ROM-Desktop,
`:634-636` + `:645` fuer jede normale Anwendung). `pstart` — der einzige Ort,
der eine neue AESPD anlegt und sie ueber `drl` in die Bereitliste haengt
(`aes/gempd.c:110-130`, `gemdisp.c:192-197` `schedule()`) — wird **nur** fuer
die internen Prozesse (`iprocess`, `geminit.c:214-227`) und fuer Accessories
(`load_one_acc`, `geminit.c:255`) gerufen. Genau so ist es bei uns:
`sched::admit` gibt es fuer Accessories (`tos-api/src/aes/acc.rs:103`), und
mehr braucht EmuTOS auch nicht.

Die Anwendung ist bei EmuTOS `rlr` **selbst** — der Kopf der Bereitliste. Was
diese Eigenschaft bei jedem Weiterreichen aufrechterhaelt, ist `disp_act`:

```c
static void disp_act(AESPD *p)          /* aes/gemdisp.c:74-79 */
{
    p->p_stat &= ~WAITIN;
    insert_process(p, &rlr);            /* EINFUEGEN, nicht voraussetzen */
}
```

Deshalb gehoert die Reparatur dorthin und nur dorthin:

* Der Programmstart ist **nicht** die strukturell naehere Nachbildung — es
  gibt dort in EmuTOS nichts nachzubilden. Ein `admit` beim Start waere eine
  NewTOS-Erfindung.
* `dsptch` bliebe auch dann falsch: es wuerde weiter Mitgliedschaft
  **voraussetzen**, und jeder Pfad, der das Bit legitim loescht — heute
  `suspend_parent_for_child`, morgen ein anderer — brauechte wieder eine
  Sonderregel. Die Invariante „wer die CPU abgibt, steht auf der Bereitliste"
  gehoert an die Stelle, die die CPU abgibt.
* Und die Tragweite ist kleiner: `dsptch` fasst **nur den laufenden Slot** an.
  Ein `admit` beim Start machte ein Programm zum `pick_next`-Kandidaten in
  Fenstern, in denen es heute keiner ist — zehn Tage vor der Vorfuehrung die
  teurere Wette.

**Beides zusammen ist nicht gemacht worden**, weil der Startpfad keinen
Quellenbeleg hat.

---

## 3. Die Aenderung

`tos-api/src/bdos/process/sched.rs`, `dsptch()` — eine Zeile Wirkcode:

```rust
PROCESS_TABLE[cur].runnable.store(true, Ordering::Release);
if let Some(sw) = on_yield(cur) { … }
```

**vor** der Auswahl, damit der Slot, zu dem umgeschaltet wird, den Aufrufer
bereits auf der Liste sieht.

`wait_cond` wird bewusst **nicht** angefasst. EmuTOS' `disp_act` loescht
`WAITIN`, weil `mwait_act` es ebenfalls ruft — fuer einen Warter, dessen
Ereignisflaggen schon gesetzt sind (`gemdisp.c:82-90`). Dieser Abkuerzung
entspricht bei uns der Leerlaufausgang von `sleep`, der den Schluessel selbst
loescht. Im `else`-Zweig, und der ist diese Funktion, ist der Aufrufer RUNNING
und sein Schluessel nach der WAITING/READY-Regel (§3.2) ohnehin `0`; der
Loeschbefehl ist dort auch bei EmuTOS wirkungslos.

**Die Warnung des Auftrags (Reihenfolge gegen `wait_cond`, PORT05/ACC10) ist
eingehalten:** `dsptch` setzt das Bit ausschliesslich fuer `running_slot()` —
den Kontext, der physisch auf der CPU ist. Ein fuer sein Kind ausgesetzter
Elter ist das nie: `suspend_parent_for_child` steht im Startpfad **vor**
`set_current(child_basepage)` (`kernel/src/arch/aarch64/exceptions.rs:1787`
und `:1794`), und `apply_switch`/`restore_kernel_park` fuehren `RUN` bei jeder
Umschaltung mit. Es wird also nichts geweckt, was schlafen soll. Genau das ist
als Testfall festgeschrieben (unten).

Mitgeaendert, weil die Kommentare den Defekt begruendeten:

* die Doku von `dsptch` und `Dispatch` in derselben Datei — der frueher dort
  stehende Satz *„will pick the caller again when it comes round"* war fuer ein
  gestartetes Programm schlicht falsch;
* `docs/api/aes/accessories.md` §3a: der Satz „`dsptch()` does not remove the
  caller from the ready list" beschreibt die Wirkung und laedt zur falschen
  Lesart ein. Ergaenzt um **„und es ist ein *Insert*, kein No-op"**, mit der
  `sh_ldapp`-Begruendung, warum EmuTOS ohne Aufnahmeschritt auskommt.

Neu: `tos-api/tests/sched_dsptch_admits_the_caller.rs` — ein `#[test]` (die
Prozesstabelle ist global), der genau den Fall festhaelt, den der ACC10-Test
nicht abdeckt, weil dessen Vorrichtung den Warter schon READY setzt:

1. Aufrufer mit `runnable == false` (das Vollbildprogramm ohne
   Ereignisschleife) → `dsptch` schaltet zum Accessory **und** nimmt den
   Aufrufer auf;
2. `wait_cond` bleibt `0`;
3. blockiert das Accessory, gibt `on_yield` die CPU an den Aufrufer zurueck —
   der Schritt, der vorher fehlte;
4. ohne jeden Kandidaten ist der Durchgang der Leerlaufpass, **und der
   Aufrufer ist trotzdem aufgenommen** (EmuTOS legt ihn auch auf eine Liste,
   auf der er allein ist);
5. **ein fuer sein Kind ausgesetzter Elter bleibt draussen**, waehrend das
   Kind `dsptch` ruft, und `resume_parent_of_child` stellt danach genau den
   gespeicherten Zustand her.

---

## 4. Die fuenf Beweise

Vollstaendig mit Protokollen und Bildern in
`measurement-2026-09-29-sched01/` (RUNINFO.TXT fuehrt sie einzeln auf;
**kein `*.img`**).

| # | Was | Beleg |
|---|-----|-------|
| 1 | **Ball-Demo beendet sich** mit beiden `.NAC`, dreimal unabhaengig; `C:\BALL.LOG` 2893 Byte und vollstaendig; Desktop mit Menuezeile zurueck und 55 s spaeter unveraendert | `raw/ball-exit-trace.log`, `-repeat2`, `-repeat3`, `raw/ball-BALL.LOG`, `screendumps/01-03` |
| 2 | **TeraDesk beendet sich weiterhin** — dieselben Aufrufnummern 49/54 wie in PORT08 Runde 2, `Pterm`, Desktop zurueck | `raw/teradesk-exit-trace.log`, `screendumps/04-06` |
| 3 | **Accessory-Weg unbeschaedigt**: *Desk* traegt beide registrierten Titel, AC_OPEN → Alert des Accessories → OK → Schirm unberuehrt zurueck, dasselbe fuer das zweite; **die vier Bilder sind bitgleich mit der ACC02-Abnahme vom 2026-09-25** | `raw/acc-desk-menu-and-launch.log`, `screendumps/07-13` |
| 4 | **Jeder Start**: `\AUTO\HELLO.NCP` (jeder Lauf), Doppelklick im C:\-Fenster startet ein GEM-Programm und der Desktop kommt zurueck, `#Z`-Autorun dreimal (BALL, TERADESK, QED), QED steht mit eigener Menuezeile | `raw/boot-fix-rpi4.log`, `raw/acc-desk-menu-and-launch.log`, `raw/qed-autorun.log`, `screendumps/14-15` |
| 5 | Basislinien + **Startbilder bitgleich auf beiden Boards** (derselbe Baum einmal mit der HEAD-Fassung von `sched.rs`, einmal mit der reparierten; `cmp` gleich, `diff` der seriellen Protokolle leer) | `raw/boot-{base,fix}-{rpi4,qemu-virt}.log`, `screendumps/16-17`, `build/` |

```
cargo run -p xtask -- test                   962 / 0 / 1     (961 + 1 neuer Test)
build --board rpi4        (frisch)           tos-api 64 / kernel 94 warnings
build --board qemu-virt   (frisch)           tos-api 64 / kernel 94 warnings
clippy kernel --features board-rpi4          tos-api 80 / kernel 125
clippy kernel --features board-qemu-virt     tos-api 80 / kernel 125
sh tools/desk-drift-probe.sh                 DRIFT-PROBE TOTAL: 32
```

**Warnungsdelta null**, in `process/sched.rs` keine einzige Clippy-Warnung,
ein Test mehr (der neue).

`sched_pexec_parent_suspend` war in einem von zwei vollen Testlaeufen einmal
rot (`an_awake_accessory_still_gets_the_cpu_while_the_child_blocks`). **Das
ist T11** — globale Prozesstabelle, parallele Threads derselben Datei; sechs
Einzellaeufe der Datei danach gruen, der zweite volle Lauf gruen. Die Datei
ruft `dsptch` nirgends auf.

---

## 5. Was der Anwender auf der Pi sehen soll

Karte wie bisher, beide Accessories drauf, Ball-Demo per `#Z` oder
Doppelklick starten:

* Die Demo laeuft wie gewohnt im Vollbild.
* **Die Quit-Taste beendet sie jetzt wirklich.** Statt des Streifens am
  oberen Rand ohne Menuezeile kommt der vollstaendige Desktop zurueck — mit
  *Desk / File / View / Options*, Laufwerkssymbolen, Papierkorb, Drucker.
* Unter *Desk* stehen die beiden Accessory-Eintraege weiterhin und lassen
  sich oeffnen und schliessen.
* `C:\BALL.LOG` ist danach vollstaendig geschrieben (nur die eine zu lange
  Zeile bleibt auf 199 Zeichen gekappt — das ist `rbuf[200]` **in der Demo**,
  PORT08 §5, und gehoert nicht uns).

---

## 6. Uebergabe an den Tester

* **Board:** `raspi4b` ist das Board, auf dem der Defekt auftreten kann —
  nur dort liegt eine Karte mit den beiden `.NAC`. `qemu-virt` ist die
  Bauprobe (Startbild + serielles Protokoll bitgleich zu vorher).
* **Was diese Aenderung beruehrt:** ausschliesslich den Weg
  `appl_exit` → `mn_cleanup` → `wait_for_accs` → `all_run` → `sched::dsptch`.
  `sched::dsptch` hat genau **einen** Aufrufer (`tos-api/src/aes/acc.rs:172`).
  Kein Startpfad, kein `Pexec`, kein `sleep`.
* **Neuer Selbsttest:** `tos-api/tests/sched_dsptch_admits_the_caller.rs`
  laeuft im Wirtsbau (`cargo run -p xtask -- test`); er ist **kein**
  `svc`-Selbsttest und braucht keine Verdrahtung im Bootpfad.
* **Nur auf echter Hardware:** nichts Spezifisches. Die Aenderung ist
  board-unabhaengig; wer sie auf der Pi sehen will, fuehrt Abschnitt 5 nach.
* **Empfohlene Gegenproben:** ACC02–ACC06-Klickliste, ein `Pexec`-Lauf
  (PEXPAR/PEXKID aus `measurement-2026-09-26-acc10/nap/`), Start aus
  `\AUTO\`, Doppelklick, `#Z`.

---

## 7. Nicht gemessen, offen benannt

* **Die echte Pi** wurde auftragsgemaess nicht gefahren.
* **Ein** Accessory statt zwei ist weiterhin nicht gefahren (wie PORT08).
* Die `H_GEMWIN`-Marke der ACC02-Klickliste vom 2026-09-25 trifft heute ein
  anderes Programm: das Symbolraster im C:\-Fenster hat sich seither
  geaendert, unter (179,187) liegt jetzt `DUMPVDI.NAP` statt `GEMWIN.NAP`.
  Der Doppelklick-**Startweg** ist damit gefahren und belegt (das Fenster
  zaehlt danach 36 statt 34 Eintraege, DUMPVDI hat geschrieben), aber das
  Bild zeigt kein laufendes Programm. Ein Lauf mit korrigierter Koordinate
  ist **nicht** nachgeholt worden.
* Kein Bild vom laufenden TeraDesk in dieser Messung — der Lauf war
  schneller als der von PORT08, beim ersten Bildabzug war er schon beendet.
  Protokoll und PORT08s Bilder decken es ab.

## 8. Ausserhalb des Auftrags beobachtet (nicht angefasst)

* Nichts Neues gefunden, das in diesen Auftrag gehoerte. (Geprueft und
  **widerlegt**: `sched::admit` loescht `wait_cond` nicht — der Slot ist beim
  Freigeben aber bereits geleert, `bdos/process.rs:969`, also kein alter
  Weckschluessel.)
* `docs/STATUS.md` ist auftragsgemaess **nicht** angefasst; der Eintrag fehlt
  dort noch und gehoert dem Lead.
