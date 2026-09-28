# PORT08 — Quittung

**Status: GEMESSEN · headless in QEMU `raspi4b` nachgestellt · nichts
repariert** · 2026-09-29 · Basis `desk-rebuild`, HEAD `6726163`.

Messbericht: `measurement-2026-09-29-port08/` (RUNINFO.TXT, drei Bildabzüge,
vier Rohprotokolle, das Messgerüst als Diff, kein `*.img`).

**Am NewTOS-Baum ist keine Quelltextzeile geändert.** Das Messgerüst lag
während der Läufe in drei Kerndateien und ist von Hand zurückgenommen;
`git status --short` und `git diff --stat` sind danach wieder der
Ausgangszustand. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren,
nichts committet, kein `git checkout/restore/clean/stash/reset`.

---

## Der letzte Aufruf

**AES Opcode 19 — `appl_exit`.** Er kehrt nicht zurück.

```
266 VDI op=101 v_clsvwk   handle=2                -> 0
267 AES op=19  appl_exit  pb=0x3ffa00
                          ctl=[19,0,1,0,0]        -> KEIN RÜCKSPRUNG
[sched] switch -> slot 0
268 AES op=23  evnt_mesag (Accessory 1) -> 0      (AC_CLOSE abgeholt)
269 AES op=23  evnt_mesag (Accessory 1) -> blockiert
[sched] switch -> slot 1
270 AES op=23  evnt_mesag (Accessory 2) -> 0      (AC_CLOSE abgeholt)
271 AES op=23  evnt_mesag (Accessory 2) -> blockiert
... danach kein einziger `svc` mehr.
```

Wire-Argumente: Selektor 200, Parameterblock `0x3ffa00`, CONTRL
`[19, 0, 1, 0, 0]` — Opcode 19, 0 `int_in`, 1 `int_out`, 0 `addr_in`,
0 `addr_out`. Das ist die Bindung, die jede GEM-Anwendung für `appl_exit()`
baut; `int_in` wird nicht gelesen. **Der Aufruf ist regelkonform.**

Die Reihenfolge des Tickets ist am Quelltext nachgeprüft
(`plat-newtos.c:604-644`) und stimmt. `v_clsvwk` ist mit `-> 0` zurück.
Der Suchraum des Nachtrags war zweistellig groß und ist damit aufgelöst.

## Warum er nicht zurückkehrt

Das Messgerüst schreibt zu jedem Aufruf die kooperative Laufmenge mit. In dem
Augenblick, in dem die Demo `appl_exit` ruft:

```
runset cur=2 : s0(st=2 run=0 ctx=1 cond=0x200000000000000)
               s1(st=2 run=0 ctx=1 cond=0x200000000000001)
               s2(st=2 run=0 ctx=0 cond=0x0)
```

**s2 ist die Demo, und `run=0`.** Sie läuft auf der CPU, steht aber nicht in
der Laufmenge. `sched::admit` — das Einzige, was `runnable` setzt — wird nur
für Accessories gerufen (`tos-api/src/aes/acc.rs:103`); ein gestartetes
GEM-Programm wird nirgends aufgenommen.

`appl_exit` → `mn_cleanup()` (`tos-api/src/aes/appl.rs:338`) weckt beide
Accessories, `wait_for_accs(AP_ACCLOSE)` → `all_run()` → `sched::dsptch()`
(`acc.rs:172`) gibt ihnen die CPU, der Aufruf der Demo bricht ab
(S2-Restartblock). Beide holen AC_CLOSE ab und blockieren wieder. Danach
sucht `pick_next` einen Slot mit `runnable == true` und findet keinen: die
Demo hat `run=0` **und** `cond=0`, ist also weder wählbar noch weckbar.
Slot 1 dreht 103 600 000 Durchgänge lang in `sleep`s Leerlauf-Ausgang, dessen
Rumpf `host().pump_input()` ist — **darum lebt die Maus.**

Die Quelle sagt es umgekehrt: EmuTOS' `disp` nimmt den laufenden Prozess vom
Kopf von `rlr` und legt ihn ohne `WAITIN` mit `disp_act` →
`insert_process(p, &rlr)` **wieder hinein** (`aes/gemdisp.c:74-79,220-238`).
Dort ist der laufende Prozess per Konstruktion auf der Bereitliste. Unser
`sched::dsptch` legt nichts ein, sondern setzt Mitgliedschaft voraus
(`tos-api/src/bdos/process/sched.rs:592-605`) — und der Kommentar dort, „will
pick the caller again when it comes round", ist für ein gestartetes Programm
falsch. Der Commit von ACC10 (`814de2c`) zitiert genau dieses
`insert_process`; der Einfügeschritt selbst ist nicht mitgekommen.

## Die Gegenprobe

Dieselbe Karte, derselbe Kern, nur die beiden Accessories entfernt:
`appl_exit` kehrt sofort zurück, die Demo schreibt ihre drei Schlusszeilen,
begeht `log_commit()`, gibt ihre neun Puffer frei, der Desktop meldet seine
Menüzeile wieder an (`AES op=30 menu_bar in=[1,…]`) und landet in
`evnt_multi`. `C:\BALL.LOG` ist 2868 Byte und endet **vollständig**
(`raw/noacc-BALL.LOG`, `screendumps/03`).

Ohne Accessory läuft `all_run()` null Runden und `all_have` ist trivial wahr —
der Pfad wird nie betreten. **Die Ursache ist damit eingekreist, nicht nur
erschlossen.**

## Der abgeschnittene `C:\BALL.LOG` — zum dritten Mal falsch gelesen

Er ist **keine Ortsangabe und auch keine Folge einer nie geschlossenen
Datei.** Die Datei wird geschlossen. Die Zeile passt nicht in den Puffer der
**Demo**: `plat-newtos.c:543` hat `static char rbuf[200]`, und `r_str` bricht
bei `rlen < sizeof(rbuf) - 1`, also bei **199** Zeichen ab. Der Text von
`write_report()` ist bei 1920×1080 201 Zeichen plus CRLF.

Die Nachbildung (`method/report-buffer-cap.py`) liefert byteweise den Schluss
von `measurement-2026-09-28-port07/raw/pi01-BALL.LOG`:

```
1920x1080 -> Fwrite length 199, tail b'pyfm -- no XBIOS screen-base cal'
640x480   -> Fwrite length 199, tail b'fm -- no XBIOS screen-base call\r'
```

Der Satz beginnt in `pi01-BALL.LOG` bei Offset 1556, ist 199 Byte lang,
1556 + 199 = 1755; `pi02` ist dort identisch. Im Trace steht
`Fwrite len=0xc7 -> 199` und `Fclose -> 0` — der Kern hat 199 von 199 Byte
geschrieben und die Datei geschlossen. Dass danach nichts mehr kommt, liegt
daran, dass der Rest des Protokolls im 32-KiB-Puffer der Demo steht und erst
das `log_commit()` **nach** `plat_close()` ihn schreibt (`ball.c:5306`).

Nebenbefund: bei 640×480 fällt nur das LF weg. Die Zeile ist also **in jeder
Auflösung** unvollständig.

## Das Urteil

| | Wessen Fehler |
|---|---|
| Der Hänger beim Beenden | **Bei uns.** `plat_close()` ruft `appl_exit()` als letzten AES-Aufruf, wie `plat-tt.c`, `plat-pistorm.c` und jede GEM-Anwendung. An der Demo ist dafür nichts zu reparieren, und eine Umgehung dort wäre auch keine: QED, TeraDesk und jede portierte Anwendung treffen dieselbe Stelle, sobald ein Accessory geladen ist. |
| Die abgeschnittene Logzeile | **Die Demo.** `rbuf[200]` ist zu klein für die eigene Zeile. Kostet den Kern nichts. |

Dass beides in derselben Datei zusammentrifft, ist der Grund, warum die
Logdatei zweimal als Ortsangabe gelesen wurde. Sie ist keine.

## Was hier nicht gemessen wurde

* **Ein** Accessory statt zwei ist nicht gefahren. Der Mechanismus hängt nicht
  an der Zahl — `s2` hat `run=0`, bevor irgendein Accessory läuft — gemessen
  ist aber nur 0 und 2.
* Der Vergleich mit dem Stand **vor** ACC10 ist nicht gefahren. Aus der Quelle
  gelesen (`814de2c^:tos-api/src/aes/acc.rs`) schlief die Barriere vorher auf
  `wait_input()`; der Slot hätte `cond != 0` gehabt und `idle_wake_pass` hätte
  ihn bei einer Mausbewegung wieder bereit gesetzt
  (`tos-api/src/bdos/process/sched.rs:704-707`). Auf der interaktiven Pi wäre
  der Hänger vorher also vermutlich von selbst weitergegangen. **Erschlossen,
  nicht gemessen** — es könnte heißen, dass ACC10 einen erholbaren Stillstand
  in einen unerholbaren verwandelt hat.
* Die echte Pi wurde auftragsgemäß nicht gefahren. Das QEMU-Bild ist dem
  geschilderten bis auf die Auflösung gleich (`screendumps/02`).
* Die übrige AES-Ausstiegsstrecke wurde nur so weit geprüft, wie der Trace sie
  zeigt.

## Abnahme des Tickets

Die Abnahme („die Demo beendet sich, der Desktop kommt vollständig zurück,
`C:\BALL.LOG` endet mit einer vollständigen Zeile") ist **nicht** erreicht und
war für diesen Auftrag auch nicht verlangt — er war eine Messung. Sie ist im
Gegenprobe-Arm ohne Accessories vollständig erreicht, was zeigt, dass zwischen
ihr und heute genau eine Sache steht.
