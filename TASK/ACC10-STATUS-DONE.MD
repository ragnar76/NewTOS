# ACC10 — Abnahmebericht

Status: **UMGESETZT, Abnahmekriterium erfuellt** (QEMU raspi4b, headless)
Beleg: `measurement-2026-09-26-acc10/`
Basis: `desk-rebuild`, HEAD `c141be2`, Aenderung **unverbaut im Baum** (nicht
committet)

---

## 1. Die Abnahme

> Ein headless-Lauf, in dem ein Programm `appl_exit` durchlaeuft und
> terminiert, ohne dass jemand die Maus bewegt.

Erfuellt. Die PORT05-Kontrolle — ein **vom Kernel** gestartetes Programm
(`#Z 01 C:\PEXKID.NAP`, kein `Pexec`) — laeuft jetzt durch. Kein Injektor,
kein Klick, keine Taste, keine Zeigerbewegung.

`diff` der beiden Serienprotokolle ist exakt sieben angehaengte Zeilen und
sonst nichts:

```
  [KID] 4 evnt_timer returned
  [sched] switch -> slot 0 / running
  [sched] switch -> slot 1 / running
< (Ende, 65 Zeilen, danach 65 s Stille)
> [sched] switch -> slot 2 / running
> [KID] 5 appl_exit done, Pterm(0)
> [launch] returned (program Pterm'd)
> [shell] sh_ldapp: 'C:\PEXKID.NAP' exited with rc=0
> [shell] sh_ldapp: starting NEWDESK, sh_nextapp=3, sh_isgem=1
> [desk] desktop up (1 drives, 0 window(s) from the context)
```

`serial/before-kernel-launch-applexit.log` gegen
`serial/after-kernel-launch-applexit.log`.

Zweites Szenario, `serial/after-pexec-child-applexit.log`: die **volle**
PORT05-Vorrichtung (EL0-Elter `Pexec`t EL0-Kind, **beide** mit echtem
`appl_exit`) laeuft ebenfalls bis zu beiden `Pterm`s und danach in den
Desktop. Genau das konnte PORT05 nicht fahren und musste es mit dem
`-DNO_APPL_EXIT`-Build umgehen; dieser Umweg ist jetzt hinfaellig.

---

## 2. Der Befund: die Zeitgrenze war es nicht (allein)

Der Auftrag nannte `WAIT_TIMEOUT 500` als den verdaechtigen Unterschied. Der
Verdacht stimmt zur Haelfte, und die **erste** Haelfte ist die wichtigere:
es geht darum, **welche Scheduler-Primitive** die Schranke benutzt.

### Was EmuTOS tut

`wait_for_accs` (`aes/geminit.c:599-628`) verbringt jeden Durchgang in
`all_run()` (`:612`), und `all_run` (`:554-566`) ist

```c
for (i = 0; i < num_accs; i++)  dsptch();        /* :559-562 */
wm_update(BEG_UPDATE); wm_update(END_UPDATE);    /* :564-565 */
```

`dsptch` (`aes/gemasm.S:129-167`) springt nach `disp`
(`aes/gemdisp.c:220-254`), und `disp` verzweigt nach dem Zustand **des
Aufrufers selbst**:

```c
p = rlr;  rlr = p->p_link;                 /* gemdisp.c:225-226 */
if (p->p_stat & WAITIN)  mwait_act(p);     /* :230  — auf nrl parken */
else                     disp_act(p);      /* :231 */
```

und `disp_act` ist `p->p_stat &= ~WAITIN; insert_process(p, &rlr);`
(`gemdisp.c:74-79`) — direkt zurueck in die Bereitliste. `ap_exit`
(`aes/gemaplib.c:251-255`) hat zwischen Eintritt und Aufruf **kein**
`ev_block`, steht also im `else`-Zweig.

**EmuTOS' Schranke ist ein begrenztes Rundlauf-*Weiterreichen*, kein Warten
auf ein Ereignis.** Sie braucht nichts als den Dispatcher und kommt auf
einer Maschine voran, die niemand anfasst.

### Was NewTOS tat

`tos-api/src/aes/acc.rs::wait_for_accs` rief
`sched::sleep(sched::wait_input())` — das ist `mwait_act`, der andere Zweig.
Der Aufrufer verliess die kooperative Laufmenge mit Weckschluessel
`WAIT_INPUT`, und nur `sched::idle_wake_pass` (Konsolenbyte oder
Zeigerzustandswechsel) konnte ihn zurueckholen. Headless gibt es beides
nicht.

Unsichtbar war das, weil interaktiv eine bewegte Maus die Klasse weckt — und
weil es vor ACC02-ACC05 ueberhaupt keine Accessories gab: `all_have` war
leer-wahr und die Schranke schlief nie.

### Und die Zeitgrenze doch auch

EmuTOS' `n` (`geminit.c:602,609`) darf eine C-Automatik sein, weil seine
Schleife die Funktion nie verlaesst: `dsptch()` kehrt an dieselbe Stelle
zurueck. Ein NewTOS-Durchgang **verlaesst** sie bei jedem Weiterreichen und
tritt oben neu ein (S2-Restart). Ein Zaehler auf dem Stapel wuerde also bei
jedem Weiterreichen zurueckgesetzt — die Vorsichtsmassnahme gegen
Endlosschleifen waere selbst eine. Der Zaehler liegt darum ausserhalb des
Rahmens, aus demselben Grund und in derselben Form wie die Zeitgrenze der
Zeitgeberwartezeit (`sched::arm_or_get_deadline`, Entwurf §4.3).

---

## 3. Spec zuerst (CLAUDE.md Regel 5)

`docs/api/aes/accessories.md` beschrieb `wait_for_accs` vollstaendig **bis
auf genau diesen Punkt**: was das "blockiert" mechanisch ist. Das war die
Luecke, durch die die falsche Primitive plausibel aussah. Vor dem Code
nachgetragen:

* **`docs/api/aes/accessories.md` §3a (neu)** — "The barrier is a *yield*
  loop, not a blocking wait": `wait_for_accs` + `all_run` im Quelltext,
  `disp`s Verzweigung, `disp_act` vs. `mwait_act`, und die drei Folgerungen,
  die Vertrag sind (Fortschritt ohne jede Eingabe; begrenzt; bei null
  Accessories null Warten *und* null Weiterreichen).
* **`docs/api/aes/accessories.md` §9.5 (neu)** — was gelandet ist, plus
  benannte Abweichung (g) zur Zaehleinheit der 500.
* **`docs/api/aes/application.md`** — `appl_exit (19)` Schritt 3 praezisiert
  (mit Verweis auf §3a) und die veraltete Notiz "in NewTOS's single-app
  bring-up there are no DAs" korrigiert; seit ACC02-ACC05 gibt es welche.

Die ACC01-Warnung des Tickets (Zeilennummern in der Gegend um ein paar
daneben) habe ich beachtet: alle Zitate oben sind gegen den gepinnten
Quelltext in `references/emutos/` nachgezaehlt, nicht aus dem Ticket
uebernommen. `geminit.c:588-630` aus dem Auftrag ist tatsaechlich
`:599-628` (Signatur bis schliessende Klammer), wie `accessories.md` §0
schon festgehalten hatte.

---

## 4. Was implementiert wurde

| Datei | Was |
|---|---|
| `tos-api/src/bdos/process/sched.rs` | **neu** `Dispatch` + `dsptch()` — die Bereitlisten-erhaltende Uebergabe, die zweite Block-Primitive neben `sleep`. Gleiche zwei Ausgaenge (`Yield(Switch)` / `Idle`); der einzige Unterschied ist, dass sie `runnable` und `wait_cond` des Aufrufers **nicht** anfasst. Deckt EL0-Slot, EL1-Kernklient und den geparkten Kernklienten ab. |
| `tos-api/src/aes/acc.rs` | **neu** `all_run()` (`geminit.c:554-566`): `count()` `dsptch`-Durchgaenge. **neu** `WAIT_PASSES`/`spend_pass`/`forget_wait`: EmuTOS' `n`, je AES-Identitaet (`pd::NUM_PDS`), ueber den S2-Restart getragen, auf beiden Ausgaengen geloescht. **geaendert** `wait_for_accs`: `all_run(); Flaggenlauf;` in der begrenzten Schleife — kein `sleep` mehr. |
| `tos-api/src/aes/mod.rs` | `release_client` loescht das Durchgangsbudget des gestorbenen Mieters, damit der naechste Bewohner des Slots die volle `WAIT_TIMEOUT` bekommt statt den Handschlag zu ueberspringen. |
| `tos-api/tests/sched_dsptch_keeps_caller_ready.rs` | **neu**, Regressionstest. |

Reihenfolge ist EmuTOS': `all_run()` laeuft **vor** dem Flaggenlauf
(`:612` vor `:617`). Das ist im `after-pexec`-Protokoll sichtbar — der Elter
gibt beiden Accessories noch eine Runde, obwohl deren `AP_ACCLOSE` laengst
gesetzt ist.

Nicht nachgebildet: EmuTOS' `wm_update(BEG_UPDATE); wm_update(END_UPDATE);`
am Ende von `all_run` (`:564-565`). Es ist dazu da, sich hinter dem Halter
der Fensterbaumsperre einzureihen; NewTOS' Sperre parkt niemanden
(`window.rs::update_beg`: *"the refusal is the one thing that is not
reproduced"*), nehmen und sofort abgeben ist hier beweisbar wirkungslos.
Steht so im Code und in §3a.

**Kein Bypass stehen gelassen.** Es gibt keinen Schalter, keinen Sonderweg
und keine Testklappe: die Schranke laeuft im Normalpfad jedes `appl_exit`.

---

## 5. Benannte Abweichung (g)

Die 500 zaehlen bei uns **Durchgaenge**, bei EmuTOS auch — aber ein
NewTOS-Durchgang, der weiterreicht, fuehrt nur **eine** von `all_run`s
`num_accs` Uebergaben aus und wird trotzdem ganz gezaehlt. Mit mehr als
einem Accessory ist unser Budget also das knappere. Beissen kann das nur in
dem Fall, in dem beide Seiten ohnehin aufgeben (ein Accessory, das sein
`AC_CLOSE` nie abholt) — und dann kehren beide zurueck, als waere die Flagge
gesetzt. In `docs/api/aes/accessories.md` §9.5 als Abweichung (g)
eingetragen.

---

## 6. Bauen, Clippy, Tests

Frisch je Board gebaut (die bekannte Falle: sequentielle Board-Builds teilen
`target/.../debug/kernel`).

```
cargo run -p xtask -- build --board rpi4        tos-api 64 / kernel 94 warnings
cargo run -p xtask -- build --board qemu-virt   tos-api 64 / kernel 94 warnings
clippy kernel --features board-rpi4             tos-api 80 / kernel 125
clippy kernel --features board-qemu-virt        tos-api 80 / kernel 125
clippy tos-api (host)                           tos-api 80
cargo run -p xtask -- test                      956 passed / 0 failed / 1 ignored
```

Grundlinie war 64/94, 80/125 und 955/0/1 — **Warnungsdelta null**, ein Test
mehr (der neue). Protokolle in `measurement-2026-09-26-acc10/build/`.

---

## 7. Uebergabe an den Tester

* **Board:** `raspi4b` ist das Board, auf dem der Defekt ueberhaupt
  auftreten kann — nur dort liegt eine Karte mit `C:\ACC\SYSACC.NAC` und
  `C:\SYSACC2.NAC` im Spiel. Auf `qemu-virt` gibt es keine Accessories,
  dort ist `count() == 0` und die Schranke kehrt sofort zurueck (der Pfad,
  der sich nicht geaendert hat).
* **Der Lauf, der diese Aenderung prueft:** `measurement-2026-09-26-acc10/
  method/seed-pex.sh` mit `method/newdesk-kidonly.inf`, dann
  `method/run-pex.sh`. Erwartung: `[KID] 5 appl_exit done, Pterm(0)` und
  danach `[desk] desktop up`. Vorher: Protokollende nach
  `[sched] switch -> slot 1 / running`.
* **Kein neuer Selbsttest zu verdrahten.** Die Regression liegt als
  Host-Test `tos-api/tests/sched_dsptch_keeps_caller_ready.rs` in
  `cargo xtask test` und laeuft dort mit. Ich habe bewusst **keinen**
  `kernel/src/selftest_*.rs` angelegt — die vorhandenen
  (`selftest_sleepwake*.rs`) haengen in keinem Bootpfad und verrotten still.
* **Regressionsflaeche, auf die zu schauen ist:** jeder Bootpfad mit
  Accessories. Das ist der raspi4b-Standardboot (zwei `.NAC` werden immer
  geladen) plus alles, was ein Programm startet und beendet — `#Z`-Autorun,
  Desktop-Doppelklick, TeraDesk/QED. Der Desktop selbst kommt in beiden
  AFTER-Laeufen hoch.
* **Nur auf echter Pi 4 pruefbar:** nichts Spezifisches. Die Aenderung
  beruehrt keine Hardware; der Unterschied QEMU/Pi ist hier allenfalls das
  Zeitverhalten der Accessories, nicht die Weckquelle.
* **Was ich NICHT gefahren habe:** nichts auf echter Hardware, keine
  Screendumps, kein `fsck_msdos`, kein interaktiver Lauf, kein
  IKBD-Injektor.

---

## 8. Ausserhalb des Auftrags beobachtet (nicht angefasst)

* `appl_exit` fehlt EmuTOS' Klammer `wm_update(BEG_UPDATE)` … `(END_UPDATE)`
  und der abschliessende `all_run()` (`gemaplib.c:253,258,259`).
* `appl_yield` (AES 17) liefert 1, ohne zu dispatchen; EmuTOS ruft dort
  `dsptch()` (`gemsuper.c:116-118`). Die Primitive dafuer gibt es jetzt.
* `appl.rs::ACCLOSE_ROUND` ist ein globales Flag, kein Flag je Identitaet;
  stirbt ein Prozess zwischen Weiterreichen und Wiedereintritt, bliebe es
  `true` und die naechste `appl_exit` uebersprunge `mn_cleanup`. Vorbestehend
  und durch diese Aenderung nicht naeher gerueckt (im Gegenteil: vorher kam
  headless ueberhaupt kein `appl_exit` mehr zurueck).
