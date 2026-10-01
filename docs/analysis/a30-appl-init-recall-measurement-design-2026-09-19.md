# A30 — Messdesign: `appl_init`-Wiederaufruf ohne Hängerrisiko

Stand 2026-09-19 · Typ: Messdesign (Entwurf, keine Implementierung) · Ticket [`TASK/A30.md`](../../TASK/A30.md)

Quellenstand: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(`VERSION_1_4-71-g1a7484ab`); NewTOS-Arbeitsbaum `desk-rebuild` @ `016cd43`, sauber.
Dumper: `tosdump/core/cases.c` (2026-09-13), `tosdump/host/build/casecheck` (2026-09-14, jünger
als `cases.c`).

**Kurzfassung.** Der Wiederaufruf ist mit dem heutigen Dumper **sicher messbar** — ohne
`appl_read`, ohne `appl_write`, ohne `appl_exit`, ohne Riegeländerung. Das Instrument ist
`evnt_multi(MU_MESAG|MU_TIMER, tmcount 0)` mit gepflanztem Puffer (seit M07 erlaubt,
`cases.c:1167-1203`, auf beiden Maschinen gefahren: `measurements/measurement-2026-09-13-m07-parameter`,
Fall 69). Die Nachrichtenquelle ist `wind_open`, das dem Eigentümer genau eine `WM_REDRAW`
einreiht (EmuTOS `gemwmlib.c:1426→1222→1030→904→862`; NewTOS `window.rs:2417→1550`). Jeder Fall
terminiert in **beiden** Ausgängen, weil der Timer-Arm den Rückweg unabhängig von der Queue
setzt (`gemevlib.c:216-220` vor `:232`; `event.rs:455-459,613-619`). Der A28-Übergang
Desktop→Kind ist mit dem Dumper **ablesbar, aber nicht herstellbar** — Abschnitt 1.3.
Hatari reicht als Referenz; der TT muss nicht mitlaufen (Abschnitt 4.3).

---

## 0. Was gelesen wurde (diese Sitzung)

EmuTOS: `aes/gemaplib.c` (`ap_init` :54-63, `ap_rdwr` :69-89, `ap_find` :95-108, `ap_exit`
:251-261), `aes/gemqueue.c` (vollständig, `doq` :31-82, `aqueue` :85-125), `aes/gemasync.c`
(`signal` :35-61, `azombie` :64-78, `evinsert` :81-93, `mwait` :117-127, `iasync` :130-179,
`apret` :182-214), `aes/gemevlib.c` (`ev_block` :64-69, `ev_mesag` :100-117, `ev_multi`
:151-294), `aes/gemsuper.c` (`crysbind` :77-166, `xif` :428-448), `aes/gempd.c` (`pd_index`
:32-35, `fpdnm` :38-57, `getpd` :60-70), `aes/struct.h` (:56, :114-117, :150-152),
`aes/gemwmlib.c` (`ap_sendmsg` :851-863, `w_redraw` :887-907, `w_update` :1004-1048,
`draw_change` :1058-1223, `wm_open` :1410-1432, `wm_close` :1438-1459, `wm_new` :1875-1893),
`aes/gemshlib.c` (:630-638, :648-658, :674-678), `desk/deskmain.c` (:2159-2166).

NewTOS: `tos-api/src/aes/event.rs` (`evnt_multi` :417-648), `appl.rs` (`appl_init` :218-240,
`appl_find` :266, `appl_write` :304-337, `appl_read` :347-395), `pd.rs` (`MsgQueue` :68-215,
`open` :301-317, `close` :477-484, `register` :603-625, `post_msg`/`post_words` :362-410),
`window.rs` (`send_redraw` :1530-1552, `wind_open` :2382-2423, `wind_new` :3142-3147,
`redraw_all`/`compose` :1200-1262), `bdos/process/sched.rs` (`arm_or_get_deadline` :376-390),
`kernel/src/desk/main.rs` (:462-490, :524-568).

Dumper: `tosdump/core/cases.c` (Riegel :397-560, Parameterriegel :860-1000, :1046-1215,
Fensterschranke :1289-1310, :1838-1849), `cases.h`, `runner_aes.c`, `record.h`, `tosdump.h`,
`gem.c`, `dumpaes.c`; Tabellen `aes-r2b-rlist.tsv`, `aes-m07-parameter.tsv`,
`aes-r4-subfunctions.tsv` (Kopf), `aes-m05-addrin.tsv` (Fälle 93-94); Messungen
`measurements/measurement-2026-09-13-m07-parameter/{RUNINFO.TXT,DIFF-…}`,
`tosdump/reference/EMUTOS14-AES-m07.LOG`; `TASK/{A06,A25,A28,A29}.md`, die A25/A28-Quittungen,
`TASK/A25-AUDIT-BEANSTANDET.MD`, `TASK/README.md`, `docs/api/aes/{application,event}.md`,
`docs/TOS-REFERENCE-DUMPER.md` §5-8, `tosdump/cases/COVERAGE-AES.md` (Kopf).

Hinweis: die im Ticket genannten Riegelzeilen `cases.c:454/:467` liegen heute bei `:482-483`
(`appl_read`), `:490` (`appl_exit`), `:495-496` (`evnt_mesag`); `evnt_multi` steht seit M07 mit
`NULL` bei `:498` und wird vom Parameterriegel `:1167-1203` beurteilt.

---

## 1. Die vier Klärungspunkte

### 1.1 Gibt es einen nicht blockierenden Weg, die Queue-Tiefe zu beobachten? — JA

**Das Instrument: `evnt_multi(MU_MESAG|MU_TIMER, tmcount = 0)` mit gepflanztem `MME_PBUFF`.**

*EmuTOS-Seite (Behauptung „blockiert nicht", belegt):*

- `crysbind` `EVNT_MULTI` (`gemsuper.c:145-159`): `count = MAKE_ULONG(MT_HICOUNT, MT_LOCOUNT)`
  = 0 (:149-150), ruft `ev_multi(MU_FLAGS & MU_TOSVALID, …, count, …, MME_PBUFF, &EV_MX)`.
- `ev_multi` (`gemevlib.c:151-294`): **Quick-Check Timer** `:216-220` — `if (tmcount == 0L)
  what |= MU_TIMER;`. Das steht **vor** `if (what == 0)` `:232`, dem einzigen Block, der
  `iasync`/`mwait` erreicht (`:232-265`). Mit `what != 0` wird er übersprungen; `:268` `ev_rets`,
  `:293` `return what`.
- **Quick-Check Message** `:222-229`: `if ((rlr->p_qindex > 0) || (rlr->p_msg.action >= 0))
  { ev_mesag(mebuff); what |= MU_MESAG; }`. Also: `int_out[0]` trägt Bit `MU_MESAG` (0x10)
  genau dann, wenn die Queue nicht leer war — und **die Nachricht wird dabei entnommen**.
- Warum `ev_mesag` auf nicht-leerer Queue nicht blockiert: `ev_mesag` (`gemevlib.c:100-117`)
  ruft `ap_rdwr(MU_MESAG, rlr, 16, mebuff)` (:116). `ap_rdwr` (`gemaplib.c:69-89`) nimmt bei
  `p_qindex == 16` den Schnellpfad (`:77-82`, `memcpy`, `p_qindex = 0`, `return 1`). Sonst
  `ev_block(MU_MESAG, &m)` (:88) → `mwait(iasync(MU_MESAG, &m))` (`gemevlib.c:64-69`) →
  `iasync` `:164-166` → `aqueue(FALSE, e, &m)` (`gemqueue.c:85-125`): `qready = (p_qindex > 0)`
  `:98-99` ist wahr → `doq(FALSE, …)` entnimmt 16 Byte (`:75-81`), `azombie(e, 1)` `:107` →
  `signal(e)` setzt `p->p_evflg |= e->e_mask` (`gemasync.c:40`). Zurück in `mwait`
  (`gemasync.c:117-127`): `!(mask & rlr->p_evflg)` ist **falsch**, also kein `dsptch()`;
  `apret(MU_MESAG)` (`:182-214`) räumt den EVB ab und liefert `e_return` = 1. Kein Warten.
- Der einzige Blockierpfad in `aqueue` ist `:120-124` (`evinsert(e, ppe)`), erreichbar nur
  mit `qready == 0`, d. h. **Lesen auf leerer** oder **Schreiben auf voller** Queue. Das Lesen
  wird vom Quick-Check `:224` nur auf nicht-leerer Queue ausgelöst. Beides zusammen: der Aufruf
  kehrt zurück, ob die Queue leer ist oder nicht.

*NewTOS-Seite:*

- `evnt_multi` (`event.rs:417-648`): `deadline = arm_or_get_deadline(now, 0)` = `now`
  (`:455-459`, `sched.rs:384-386`); in der ersten Schleifenrunde `if flags & MU_TIMER != 0 &&
  monotonic_millis() >= deadline { fired |= MU_TIMER }` (`:613-615`) → `if fired != 0 { break }`
  (`:617-619`) → `set_ret(fired)` (`:646`). Der Block `sched::sleep(wait_input())` (`:631`) wird
  nie erreicht. `MU_MESAG`: `src.queue.pop()` (`:598`) → `fired |= MU_MESAG` (`:609`) — pop ist
  destruktiv wie `doq`.
- `evnt_timer(0)`/`(1)` sind auf der **echten Pi** gelaufen (`aes-r1.tsv:228-229`, run1-pi4) über
  dieselben Primitive `arm_or_get_deadline`/`monotonic_millis` (`event.rs:751-764`).
  `evnt_multi` selbst ist auf NewTOS bisher nur in QEMU `raspi4b` gefahren (M07 Run B). Das ist
  eine **Inferenz** für die echte Pi, keine Messung — siehe Abschnitt 6.

*Riegel:* `cases.c:1167-1203` lässt Opcode 25 genau in dieser Form durch (alle 16 `int_in`
gesetzt, `IN_LEN 16`, `MU_TIMER` mit `MT_LOCOUNT/MT_HICOUNT == 0`, bei `MU_MESAG` ein
`a0=`-Plant). Für Preludes gilt derselbe Riegel (`:1040-1045`).

*Bereits gemessen:* `aes-m07-parameter.tsv:69` (`48,0,…`, `a0=0`, `!reset none`, keine Fenster)
→ `int_out[0] = 32` auf **beiden** Maschinen („nothing queued, so ev_mesag is not reached",
`measurements/measurement-2026-09-13-m07-parameter/RUNINFO.TXT`). Damit ist der leere Zweig gemessen; der
nicht-leere Zweig (Bit 0x10 gesetzt, Rückkehr ohne Warten) ist bisher nur aus der Quelle belegt
— Fall **C1** des Entwurfs macht daraus eine Messung, **bevor** der eigentliche Messfall gelesen
wird.

**Tiefe statt Existenz:** weil jeder Aufruf genau eine Nachricht entnimmt, ist der k-te
aufeinanderfolgende Aufruf genau dann `48`, wenn die Tiefe ≥ k war. Zwei Reads hintereinander
(einer als Prelude, einer gemessen) messen „mindestens zwei", drei messen „genau zwei". Das ist
die Differenzmessung, die das Ticket unter Punkt 2 sucht.

**`appl_write` (12) — verrät der Rückgabewert die Tiefe? NEIN, und es ist als Quelle unbrauchbar:**

- Rückgabe: `aqueue(TRUE, …)` `gemqueue.c:96-97` `qready = (qpb_cnt <= QUEUE_SIZE - p_qindex)`;
  wahr → `doq` + `azombie(e, 1)` `:104-107` („ap_rdwr() will return 1 => OK"); **falsch → Block**
  `:120-124`, und da Sender und Empfänger derselbe Prozess sind, endet der nie. Der Rückgabewert
  ist also **immer 1 oder ein Hänger** — er verrät nichts über die Tiefe unterhalb von 8
  (`QUEUE_SIZE 128`, `struct.h:56`, / 16 Byte). NewTOS liefert auf voller Queue **0**
  (`pd.rs:166-168`, `appl.rs:336`) — genau die Asymmetrie, für die der Riegel existiert.
- Ziel-PID: `AP_RWID = int_in[0]` (`gemsuper.c:103-104`), muss die **eigene** `ap_id` sein. Die
  ist maschinen- und startpfadabhängig: TT/Hatari **0** (`EMUTOS14-AES-m07.LOG`: „appl_init id
  0"), NewTOS per `#Z`-Autorun **1** (M07 Run B), per Desktop-Doppelklick **2** (A06, run1-pi4).
  Eine statische Tabelle kann sie nicht nennen. Eine falsche PID liefert `fpdnm(NULL, pid)` =
  NULL (`gempd.c:38-57`), und `ap_rdwr(MU_SDMSG, NULL, …)` → `aqueue` liest `p->p_qindex` durch
  NULL (`gemqueue.c:94-97`) und `doq` schreibt über `p->p_qaddr` (`:39`) — auf dem TT ein
  Supervisor-Zugriff in den Vektorbereich, kein Fehlercode. NewTOS antwortet 0
  (`appl.rs:310-313`). Der Opcode-Riegel hat für 12 **keinen** Parameterarm (`cases.c:484`,
  `NULL`); die r4-Tabelle hält ihn nur per Hand fern (`aes-r4-subfunctions.tsv:116`).
- Folge: `appl_write` kommt in diesem Entwurf **nicht** vor. Die Nachrichtenquelle ist
  `wind_open` — sie braucht keine PID, schreibt an `D.w_win[h].w_owner` (`gemwmlib.c:892,904`)
  und ist in jeder Fenstertabelle auf beiden Maschinen gelaufen.

**`appl_find` (13):** `ap_find` (`gemaplib.c:95-108`) liefert `p->p_pid` oder −1 und liest die
Queue nicht — für die Tiefe wertlos. Es könnte die eigene PID liefern (Name „DUMPAES ", per
`p_nameit`), aber die Tabelle hat keinen Datenfluss von einem Prelude-Ergebnis in den nächsten
Aufruf (`runner_aes.c`: jedes Prelude wird aus seiner Zeile gelegt). Unbenutzt.

### 1.2 Falls nur `appl_read` bliebe: die Zwei-Nachrichten-Konstruktion — nicht nötig, aber tragfähig

Es bleibt nicht nur `appl_read`; 1.1 liefert ein nicht blockierendes Instrument. Die
Differenz-Idee des Tickets ist trotzdem übernommen, weil sie **die Diskriminierung schärft**:

- **M3**: `open 1` → `WM_REDRAW(1)`; `appl_init`; `open 2` → `WM_REDRAW(2)` (nach dem
  Wiederaufruf); Prelude-Read; gemessener Read. Gutfall: beide da → zweiter Read `48`.
  A25-Defekt (Queue beim Wiederaufruf geleert): nur `WM_REDRAW(2)` → erster Read nimmt sie,
  zweiter `32`. Die Queue ist **beim ersten Read nie leer** — genau die vom Ticket verlangte
  Garantie, hier aber ohne dass sie für die Terminierung gebraucht würde.
- Voraussetzung, belegt: das Öffnen von 2 sendet **keine** zweite `WM_REDRAW(1)`. `draw_change`
  setzt für ein Open `start = w_handle`, `stop = DESKWH` (`gemwmlib.c:1097-1098`), Open-Zweig
  `:1157-1168` (`rc_copy(pt, &c)`), dann `w_update(start, &c, stop, FALSE)` (`:1222`): mit
  `bottom == top == w_handle` läuft die Schleife `:1024-1044` genau einmal, `w_redraw(top)`
  nur für das geöffnete Fenster. NewTOS `wind_open` → `send_redraw(handle, w.work)` (`window.rs:2417`)
  ebenfalls nur für das geöffnete. Und `doq` vereinigt `WM_REDRAW` nur bei **gleichem** Handle
  (`gemqueue.c:52-56`; `pd.rs:154-156`), zwei Handles bleiben zwei Nachrichten.
- Fall **M4** (dritter Read) belegt zusätzlich „genau zwei, nie drei".

`appl_read` selbst bleibt gesperrt (`cases.c:482-483`) und wird nicht gebraucht: es lieferte
dieselbe Information (Existenz/Tiefe über den Rückgabewert), und den Nachrichten-**Inhalt**
sieht der Record so oder so nicht (Abschnitt 2). Nebenbefund: NewTOS' `appl_read` blockiert
**nie** (`appl.rs:366-369`, leer → 0), EmuTOS' blockiert auf leerer Queue — ein
`appl_read`-Fall liefe auf der Pi in jedem Ausgang sauber und hinge nur auf dem TT; das ist
exakt die Asymmetrie, die den Riegel begründet.

### 1.3 Der A28-Übergang Desktop → Kind — ablesbar, nicht herstellbar

Was A28 verlangt: vor dem Start eines GEM-Kindes leert der Desktop seine Queue
(`deskmain.c:2166` `appl_exit()` → `ap_exit` `gemaplib.c:256-257`), und `sh_ldapp` löscht
die Flags (`gemshlib.c:636`). NewTOS: `desktop_relinquish` → `appl_exit()` (`desk/main.rs:566`),
`pd::register` → `flags = 0` (`pd.rs:619`).

Der Dumper **ist** das Kind. Was er messen kann, ist der Zustand seiner Queue **beim Start** —
Fall **P1** (erster Fall der Tabelle, keine Preludes, gemessener Read): `48` heißt „eine
Nachricht war schon da, bevor der Dumper irgendetwas eingereiht hat", also geerbt. **P2** sagt,
ob es mehr als eine war.

Was er **nicht** kann: die Vorbedingung herstellen. Eine geerbte Nachricht setzt voraus, dass
der Desktop eine **ungelesene** `WM_*` hatte, als er losließ (A28-Status §7: verdecktes Fenster
anklicken, dann Doppelklick). Das ist eine Bedienergeste **vor** dem Start des Dumpers; kein
Prelude, kein Reset und kein Opcode der Tabelle erreicht diesen Zeitpunkt. Der Dumper darf auch
kein Programm starten (`shel_write` FATAL), und ein Kettenstart über `Pexec` träfe einen
**anderen** Deskriptor (`allocate_slot` nimmt den nächsten freien Slot; der Desktop-Fall teilt
sich den Slot nur, weil der Desktop ohne BASEPAGE `pid 1` bekommt, `appl.rs:191-199`,
`mod.rs:504-506` laut A25-Audit).

Was daraus folgt:

- **Referenzwert P1/P2 = 32/32**, aus Quelle (`deskmain.c:2166`) und Messung (M07 Fall 69 auf
  Hatari: leere Queue beim ersten `MU_MESAG`-Read eines frisch gestarteten Programms).
- **Auf der Pi ist P1 ein bedingter Fall**: im `#Z`-Autorun-Pfad (der `desktop_relinquish` nie
  betritt, A28-Status §6) ist 32 in jedem Kernelzustand zu erwarten — Kontrollfall. Nur der
  Doppelklick-Start **mit** der Geste aus A28-Status §7 Schritt 1-2 macht P1 zur Aussage:
  `48` = Kind hat geerbt (A28-Defekt bei vorhandenem A25-Fix — genau die Kombination B1 des
  Audits), `32` = Grenze leert. Die Geste muss im `RUNINFO.TXT` protokolliert sein; ohne sie ist
  ein 32 in P1 kein Beleg für A28.
- **Einschränkung der Lesart von P1**: 48 zeigt „fremde Nachricht", nicht „von wem". Der Record
  sieht den Inhalt nicht (Abschnitt 2); ein Fensterhandle, das der Dumper nie geöffnet hat, wäre
  nur mit der Puffer-Erfassung aus Folgeauftrag F1 sichtbar. Und P1 setzt voraus, dass beim
  Dumperstart kein Desktop-Fenster mehr in der lebenden Tabelle ist — das A25-Audit nennt dafür
  `save_windows()` (`window.rs:805-822`, nicht in dieser Sitzung selbst gelesen). Wäre doch eines
  offen, könnte der Tabellen-Reset `wind_new` es schließen und dabei selbst eine `WM_REDRAW` an
  `pid 1` = den Dumper senden — ein 48 in P1 wäre dann zweideutig.
- **A28 ist mit dem Dumper allein also nicht sicher messbar** (Abnahmepunkt 3). Der Entwurf
  macht die vorhandene Bedienerabnahme aus A28-Status §7 **objektiv ablesbar** (eine Zahl im
  Record statt „QED zeichnet ins falsche Rechteck"), mehr nicht. Folgeauftrag F3 in Abschnitt 5.

### 1.4 Kommt der Fall ohne `appl_exit` aus? — JA

- `ap_init` hat keine Vorbedingung; es setzt nur `p_flags |= AP_OPEN` und liefert `p_pid`
  (`gemaplib.c:54-63`). Ein zweiter, dritter Aufruf ohne `appl_exit` dazwischen ist idempotent.
  NewTOS `pd::open` findet den Slot per PID und ORt nur (`pd.rs:301-317`, A25-Fix).
- Der Dumper ruft `appl_exit` **selbst**, außerhalb der Fälle: `td_gem_close` (`gem.c:117-127`)
  nach `td_run_aes`, in `dumpaes.c:157`. Sein `ap_exit` (`gemaplib.c:251-261`) leert eine
  eventuell volle Queue über `ap_rdwr(MU_MESAG, rlr, p_qindex, …)` (:256-257) — mit `qready`
  wahr, also ohne Block; `wait_for_accs(AP_ACCLOSE)` (:255) ist in allen bisherigen Läufen
  zurückgekehrt (keine Accessories). Der Entwurf lässt die Queue nach dem letzten Fall ohnehin
  leer (Abschnitt 3.4, Rückbau).
- `appl_exit` (19) bleibt FATAL im Riegel (`cases.c:490`) und wird von keiner Zeile genannt.

---

## 2. Das Instrument: was der Record sieht und was nicht

Der AES-Record ist 120 Byte (`tosdump.h:104-125`): `contrl[5]`, `global[15]`, `int_in[16]`,
`int_out[7]`, `addr_in[3]` (32 Bit), `addr_out[1]`, `ret`. **Kein Feld trägt den Inhalt eines
gepflanzten Puffers**; ein gepflanzter Slot trägt das Token `0xB0FF0000+n`
(`tosdump.h:174`, `runner_aes.c:308-311`). Die 16 Byte Nachricht, die `ev_mesag`/`pop` in den
Puffer schreibt, sind damit **unsichtbar**. Das ist seit M05 bekannt und dort als offen benannt
(STATUS 2026-09-12: „der 120-Byte-AES-Record hat keinen Platz für den INHALT eines Puffers").

Was der Entwurf misst, ist deshalb **`int_out[0]` von `evnt_multi`**: 32 (`MU_TIMER`) gegen 48
(`MU_TIMER|MU_MESAG`). Für die A25-Frage („überlebt die Nachricht den Wiederaufruf") reicht das,
weil Existenz und Tiefe die Frage sind, nicht der Inhalt. Für P1 (welche Nachricht kam von wem)
reicht es **nicht** — Folgeauftrag F1.

Was **konstruktionsbedingt** abweicht und im Tabellenkopf deklariert gehört (wie M07):
`int_out[1..4]` (Mausposition, Tastenbyte, Shift-Status: TT 319/239 gegen Pi 320/240 in jedem
M07-Fall); `global[2]`, `[3]`, `[4]`, `[10]`, `[11]`, `[12]` (wie in jeder Runde); bei **R0**
zusätzlich `int_out[0]` = `ap_id` (0 gegen 1/2). `OUT_LEN 5` auf jedem `evnt_multi`, weil
`int_out[5..6]` mit `what != 0` nicht geschrieben werden und `xif` sonst Stapel kopiert
(`gemsuper.c:430-432,444-445`; M07-Kopf).

---

## 3. Der Fallentwurf

### 3.1 Bausteine (jeweils eine Tabellenzeile)

| Kürzel | Zeile | Zweck |
|---|---|---|
| `D` (Drain) | `+25 16 5 1 0 48,0×15 a0=0` | eine Nachricht entnehmen, falls da; kehrt immer zurück |
| `W1` | `+100 5 1 0 0 0x0FEF,10,20,300,200` / `+101 5 1 0 0 1,30,40,320,240` | Fenster 1 erzeugen/öffnen → **eine** `WM_REDRAW(1)` |
| `W2` | `+100 …` / `+101 5 1 0 0 2,60,80,200,160` | Fenster 2 darüber → **eine** `WM_REDRAW(2)` |
| `I` | `+10 0 1 0 0 - -` | `appl_init` erneut |
| `R` (gemessen) | `25 16 5 1 0 48,0×15 a0=0` | der Read, dessen `int_out[0]` gemessen wird |

Geometrie und `kind` sind die von `aes-r2b-rlist.tsv` (auf beiden Maschinen gefahren). Nur
Handles 1 und 2 (Schranke `TD_AES_MAX_OPEN_WINDOWS 4`, `cases.c:1289-1310`).

**Warum acht Drains vor jedem konstruierenden Fall:** die Queue fasst 8 Nachrichten
(`QUEUE_SIZE 128` / 16; `pd.rs:50`). Acht Reads leeren sie aus **jedem** Vorzustand, und
zwischen den Drains reiht nichts ein (kein Bediener, keine Fenster). Ein Drain auf leerer
Queue kostet nichts (Quick-Check nimmt den Message-Zweig gar nicht). Damit ist der Zustand vor
`W1` in jedem Fall **leer und stated** — die Zustandsreset-Forderung des Tickets, gelöst durch
Preludes, weil `wind_new` die Queue nicht anfasst (`gemwmlib.c:1875-1893`).

**Schranke „nie voll" (der einzige EmuTOS-Hängerpfad beim Schreiben):** nach dem Drain ist die
Tiefe 0; `W1` → 1; `I` → 0 oder 1; `W2` → ≤ 2. Der Reset vor dem nächsten Fall schließt 1 vor 2
(`wm_new` `:1886-1891`), `wm_close(1)` sendet dabei höchstens eine `WM_REDRAW(2)` (`:1453` →
`w_update` für die noch offenen), danach `wm_close(2)` nichts mehr → beim nächsten Fall ≤ 2 vor
dem Drain. Ein Write blockiert erst bei `p_qindex > 112` (`gemqueue.c:97`). Unerreichbar.

### 3.2 Fallfolge, Erwartungswerte, Terminierung

Reset `!reset wind_new` läuft vor **jedem** Fall (`runner_aes.c:236`) und einmal nach dem
letzten (`:333`). Tabellenform `!columns 8` (für `a0=`).

| # | Preludes | gemessen | Gutfall | A25-Defekt (Queue beim Wiederaufruf geleert) | Zweck |
|---|---|---|---|---|---|
| **P1** | — | `R` | 32 | 32 (A28-Defekt + Geste: **48**) | Queue beim Start (1.3) |
| **P2** | — | `R` | 32 | 32 (≥ 2 geerbt: 48) | Tiefe beim Start |
| **R0** | — | `10 0 1 0 0` | `int_out[0]` = `ap_id`, `global[0..12]` wie in der LOG-Zeile „global after appl_init" | dito | der Wiederaufruf selbst: gleiche Id, sechs Globals neu geschrieben |
| **C0** | `D×8` | `R` | 32 | 32 | acht Drains leeren |
| **C1** | `D×8 W1` | `R` | **48** | 48 | das Instrument sieht die eine `WM_REDRAW(1)` — Nachweis des nicht-leeren Zweigs, **vor** M1 |
| **C2** | `D×8 W1 D` | `R` | 32 | 32 | genau eine Nachricht pro Open |
| **M1** | `D×8 W1 I` | `R` | **48** | **32** | **die Messung**: Nachricht überlebt `appl_init` |
| **M2** | `D×8 W1 I I` | `R` | 48 | 32 | zwei Wiederaufrufe |
| **M3** | `D×8 W1 I W2 D` | `R` | **48** | **32** | Differenz (1.2): zwei da oder nur die zweite |
| **M4** | `D×8 W1 I W2 D D` | `R` | 32 | 32 | Kontrolle zu M3: genau zwei, nie drei |
| **M5** | `D×8 I W1` | `R` | 48 | 48 | eine **nach** dem Wiederaufruf eingereihte Nachricht kommt an (Deskriptor lebt, PID gleich) |

Reihenfolge ist Pflicht für P1/P2 (**vor** allem, was liest oder — bei Defekt — löscht: R0
würde unter dem A25-Defekt eine geerbte Nachricht vernichten). Alles andere ist frei.

**Terminierungsnachweis je Baustein — gilt für jeden Fall, in jedem Ausgang:**

- `wind_new` (Reset): `wm_new` schließt/löscht ohne Warten (`gemwmlib.c:1875-1893`); in r2, r2b,
  r3, r4 auf beiden Maschinen vor jedem Fall gelaufen. NewTOS `wind_new` (`window.rs:3142-3147`)
  malt und postet nichts (`compose` `:1211-1262`).
- `D`/`R`: Abschnitt 1.1 — EmuTOS `gemevlib.c:216-220` setzt `what` vor `:232`; auf nicht-leerer
  Queue ist `aqueue` mit `qready` wahr (`gemqueue.c:98-107`) und `mwait` ohne `dsptch`
  (`gemasync.c:120`). NewTOS `event.rs:613-619`. **Unabhängig davon, ob die Queue leer ist** —
  das ist der Kern: der Fehlerfall (Queue geleert) ist der **leere** Zweig, und der ist der
  gemessene Zweig aus M07 Fall 69.
- `W1`/`W2`: `wm_open` (`gemwmlib.c:1410-1432`) → `ap_sendmsg` → `ap_rdwr(MU_SDMSG)` → `aqueue(TRUE)`
  mit `qready` wahr solange ≤ 8 (3.1). `wm_update(BEG_UPDATE)` (:1422) blockiert nur bei fremdem
  Lock — ein Prozess. In r2b/r3/r4 auf beiden Maschinen gelaufen. NewTOS `post_msg` liefert bei
  voller Queue `false` statt zu blockieren (`pd.rs:166-168`).
- `I`: `ap_init` `gemaplib.c:54-63` — drei Anweisungen, kein Warten. NewTOS `appl_init`
  `appl.rs:218-240` — kein Warten, keine Bedingung außer freiem/gefundenem Slot; der Slot ist
  gefunden (gleiche PID wie beim ersten `appl_init` aus `gem.c:90`).
- R0: wie `I`, gemessen.

**Was der Entwurf am A25-Defekt genau unterscheidet:** M1/M2/M3 sagen 48 gegen 32. C1 stellt
sicher, dass ein 32 in M1 nicht „Instrument blind" bedeutet; C2/M4 stellen sicher, dass ein 48
nicht „mehr Nachrichten als gedacht" bedeutet; M5 trennt „Wiederaufruf löscht" von „Wiederaufruf
tauscht den Deskriptor" (dann wäre auch M5 32).

### 3.3 Gemessene Felder

Je Fall der volle 120-Byte-Record. **Vergleichsrelevant:** `int_out[0]` (alle `R`-Fälle),
`int_out[0]` + `global[0..12]` (R0), `addr_in[0]` = Token `B0FF0000` (alle `R`-Fälle; ein anderer
Wert hieße „der Aufruf hat den Slot beschrieben"), `int_in[0..15]` unverändert (Dispatcher fasst
`int_in` nicht an, `gemsuper.c:437`). **Konstruktionsbedingt verschieden**, deklariert:
`int_out[1..4]`, `global[2,3,4,10,11,12]`, bei R0 `int_out[0]`.

### 3.4 Zustandsreset und Rückbau

- Zwischen den Fällen: `wind_new` (Fenster) + `D×8` (Queue). Nach dem letzten Fall (M5, Queue
  leer nach dem Read, Fenster 1 offen) läuft der Schluss-Reset: `wm_close(1)` ohne weiteres
  offenes Fenster → keine Nachricht. `td_gem_close` → `v_clsvwk`, `appl_exit` → `ap_exit` findet
  `p_qindex == 0` (:256) und überspringt den Read.
- `AP_MESAG` wird von jedem `MU_MESAG`-Read gesetzt (`gemsuper.c:147-148`; NewTOS nur bei
  erfolgreichem Pop, `event.rs:608`). In EmuTOS liest es nur `geminit.c:737` (Accessories); im
  Record nicht sichtbar; kein Rückbau nötig.
- Die Tabelle hinterlässt nichts, was `gemshlib.c:674-677` nicht ohnehin wegräumte.

---

## 4. Tabelle, Laufverzeichnis, Referenz

### 4.1 `tosdump/cases/aes-r5-applinit.tsv` — Vorschlag, **nicht angelegt**

Der Entwurf wurde im Sitzungs-Scratchpad mit dem Host-Lader geprüft (Nur-Lese-Beobachtung,
kein Artefakt im Repo): `casecheck --aes` → **Exit 0, „11 cases, 92 preludes, 8 columns"**;
Opcode 10 als Prelude, Opcode 25 mit `a0=`-Plant als Prelude und als Fall, Handles 1/2 — alles
angenommen. Fall-Ids und CRC entstehen erst mit der wirklich geschriebenen Datei (der Kopf
wächst noch); die Nummern unten sind die des Scratch-Entwurfs (P1=29, P2=30, R0=36, C0=50,
C1=62, C2=75, M1=92, M2=106, M3=122, M4=139, M5=152) und nur zur Orientierung.

Vollständiger Entwurf (Tab-getrennt; `D` steht für die Zeile
`+25<TAB>16<TAB>5<TAB>1<TAB>0<TAB>48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0<TAB>a0=0<TAB>drain n`,
achtmal wiederholt):

```
# aes-r5-applinit.tsv -- AES round 5: appl_init called again (A25/A30).
# Format: eight tab separated columns (!columns 8):
#   opcode  n_intin  n_intout  n_addrin  n_addrout  intin  addr_in  comment
# The read that never blocks: evnt_multi(MU_MESAG|MU_TIMER, tmcount 0) with a
# planted MME_PBUFF (int_in[0] = 48).  The quick check timer sets `what`
# before the wait block (gemevlib.c:216-220 vs :232-262), so the call returns
# whether or not the queue holds a message; int_out[0] carries MU_MESAG (0x10)
# iff one was there -- and that one is dequeued.  32 = empty, 48 = one taken.
# The message source that needs no pid: wind_open sends the owner ONE
# WM_REDRAW for the opened handle (gemwmlib.c:1426->1222->1030->904->862;
# NewTOS window.rs wind_open -> send_redraw).  Handles 1 and 2 only, the
# aes-r2b-rlist.tsv geometry.
# Differs by construction: int_out[1..4] (mouse/keys), global[2,3,4,10,11,12],
# and int_out[0] of the measured appl_init (ap_id: 0 on the TT, 1 or 2 here).
# opcode\tn_intin\tn_intout\tn_addrin\tn_addrout\tintin\taddr_in\tcomment

!reset	wind_new
!columns	8

# P. The queue as the run finds it -- BEFORE anything drains it.
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	P1 queue at start: a message here was inherited across the launch boundary
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	P2 queue at start, second read: was there more than one

# R. The re-call itself, measured.
10	0	1	0	0	-	-	R0 appl_init a second time: same id, six globals rewritten

# C. Instrument controls, no re-call.
D×8
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	C0 after eight drains the queue is empty: 32

D×8
+100	5	1	0	0	0x0FEF,10,20,300,200	-	create 1
+101	5	1	0	0	1,30,40,320,240	-	open 1: one WM_REDRAW(1) queued
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	C1 the instrument sees the one message wind_open queued: 48

D×8
+100	5	1	0	0	0x0FEF,10,20,300,200	-	create 1
+101	5	1	0	0	1,30,40,320,240	-	open 1
+25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	read 1 takes WM_REDRAW(1)
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	C2 exactly one message per open: 32

# M. The measurement: a message queued BEFORE appl_init is called again.
D×8
+100	5	1	0	0	0x0FEF,10,20,300,200	-	create 1
+101	5	1	0	0	1,30,40,320,240	-	open 1: WM_REDRAW(1) queued
+10	0	1	0	0	-	-	appl_init again
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	M1 the message survives the re-call: 48 (A25 defect: 32)

D×8
+100	5	1	0	0	0x0FEF,10,20,300,200	-	create 1
+101	5	1	0	0	1,30,40,320,240	-	open 1
+10	0	1	0	0	-	-	appl_init again
+10	0	1	0	0	-	-	and once more
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	M2 two re-calls in a row: 48 (A25 defect: 32)

D×8
+100	5	1	0	0	0x0FEF,10,20,300,200	-	create 1
+101	5	1	0	0	1,30,40,320,240	-	open 1: WM_REDRAW(1) queued
+10	0	1	0	0	-	-	appl_init again
+100	5	1	0	0	0x0FEF,10,20,300,200	-	create 2
+101	5	1	0	0	2,60,80,200,160	-	open 2 on top: WM_REDRAW(2) queued AFTER the re-call
+25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	read 1: never empty, whichever way the re-call went
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	M3 the difference: 48 if both survived, 32 if only WM_REDRAW(2) did

D×8
+100	5	1	0	0	0x0FEF,10,20,300,200	-	create 1
+101	5	1	0	0	1,30,40,320,240	-	open 1
+10	0	1	0	0	-	-	appl_init again
+100	5	1	0	0	0x0FEF,10,20,300,200	-	create 2
+101	5	1	0	0	2,60,80,200,160	-	open 2
+25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	read 1
+25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	read 2
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	M4 control for M3: exactly two, never three: 32 either way

D×8
+10	0	1	0	0	-	-	appl_init again, BEFORE any message
+100	5	1	0	0	0x0FEF,10,20,300,200	-	create 1
+101	5	1	0	0	1,30,40,320,240	-	open 1 after the re-call
25	16	5	1	0	48,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0	a0=0	M5 a message queued after the re-call still arrives: 48 either way
```

Beim Anlegen: den Kopf um die Deklaration der konstruktionsbedingten Abweichungen und die
P1-Lesart (1.3) ergänzen, dann **einmal** schreiben; CRC bindet. Die eingefrorenen Tabellen
bleiben unberührt (Abnahmepunkt 4).

### 4.2 Laufverzeichnis

`measurements/measurement-2026-09-<TT>-a30-applinit/` mit `RUNINFO.TXT` (Tabelle, SHA/CRC, Dumper-Build,
Kernel-SHA, **Startpfad je Lauf** und — beim Doppelklick-Lauf — die Geste aus A28-Status §7),
Unterverzeichnisse `emutos14-tt-hatari/`, `newtos-pi4-autorun/` (`#Z`, Kontrolle für P1),
`newtos-pi4-dblclick/` (Doppelklick mit Geste; der eigentliche A28-Ablesefall), optional
`newtos-qemu-raspi4b/` (640×480-Profil wie die TT, nach dem M07-Muster). Referenzkopie nach
`tosdump/reference/EMUTOS14-AES-r5applinit.{BIN,LOG}` (Namensmuster `EMUTOS14-AES-r2brlist`).
Vergleich mit `tosdiff` → `DIFF-emutos-vs-newtos-*.txt`, Rohdiff unverändert ablegen.

**Reihenfolge des Fahrens:** zuerst Hatari (ein Hänger dort kostet nur den
`--run-vbls`-Abbruch), dann QEMU, dann Pi. Erst wenn Hatari „all 11 cases ran" meldet, ist der
nicht-leere Zweig (C1) auch auf EmuTOS gemessen und nicht nur gelesen.

### 4.3 Reicht Hatari als Referenz? — JA, der TT muss nicht mitlaufen

- Hatari 2.6.1 mit `etos512de.img` ist gegen zwei echte TT-Dumps **bitgleich** validiert
  (`aes-r2b-rlist` 15 Fälle, `aes-r2-wind` 78 Fälle; STATUS-Eintrag 2026-09-12); die
  Referenzen für M05, M06, M07 stammen ausschließlich aus Hatari.
- Das Hatari-ROM ist das 1.4-Release; zwischen `VERSION_1_4` und dem Pin sind
  `aes/gemaplib.c`, `gemqueue.c`, `gemevlib.c`, `gemasync.c`, `gemsuper.c`, `gempd.c` **unverändert**
  (`git log VERSION_1_4..HEAD -- …` in `references/emutos`: nur `gemwmlib.c` `WF_OWNER` und
  `deskmain.c` `shel_wdef`, beide für diesen Pfad unerheblich). Die Zitate oben gelten also
  für das gemessene ROM.
- Rezept wie M07 Run A (`RUNINFO.TXT`): `SDL_VIDEODRIVER=dummy Hatari --machine tt --tos
  etos512de.img --tos-res ttmed --monitor vga --sound off --fast-forward on --run-vbls 20000
  --conout 2 -d <dir> --gemdos-drive C --auto 'C:\DUMPAES.PRG'`, ~13 s.

---

## 5. Was nicht sicher messbar ist — Folgeaufträge (eigene Tasks, keine stillen Erweiterungen)

**F1 — Puffer-Erfassung im AES-Record (optional für A25, nötig für A28-Lesart).** Der
120-Byte-Record (Format 2) trägt keinen Pufferinhalt. Ein Feld für die ersten 16 Byte des in
`addr_in[0]` gepflanzten Puffers **nach** dem Aufruf machte sichtbar: Nachrichtentyp (20 =
`WM_REDRAW`), Handle (in P1: eines, das der Dumper nie geöffnet hat), und in M3, **welche** der
beiden überlebt hat. Kosten: neue Record-Version oder Sidecar-Datei, `record.c`/`tosdiff`
anpassen; alle bestehenden AES-Dumps bleiben an Format 2 gebunden (Mischung verboten). Nicht
vor der Demo.

**F2 — `@apid`-Ersetzung in der `int_in`-Spalte (optional).** Ein Token, das der Runner zur
Laufzeit durch `global[2]` ersetzt, machte `appl_write` an sich selbst und `appl_read` mit
eigener Id aus einer statischen Tabelle heraus möglich; der Record müsste dort ein Token statt
des Werts ablegen (wie `TD_AES_ADDR_TOKEN`). Für A25 nicht nötig (1.1); erst sinnvoll zusammen
mit einem Parameterarm für Opcode 12 (siehe Nebenbeobachtung N1).

**F3 — A28-Übergang: Instrument außerhalb des Dumpers.** Der Dumper kann die Vorbedingung
„Desktop hat eine ungelesene `WM_*`" nicht herstellen (1.3). Bis auf Weiteres ist das Verfahren:
Bedienergeste nach A28-Status §7 Schritt 1-2 auf der echten Pi + Doppelklick auf
`DUMPAES.NAP` + Ablesen von P1 (`newtos-pi4-dblclick/`). Ein automatisierbares Instrument
bräuchte einen desktop-seitigen Haken (skriptierter Fensterklick vor einem Autostart) — Kernel-
bzw. Desktop-Arbeit, nicht Dumper-Arbeit, und nach Memory „Desktop interactive verification:
real Pi4 HW only" derzeit ohnehin nur am Gerät. Eigener Task, Priorität nach der Demo.

---

## 6. Zitiert gegen gefolgert

**Zitiert (in dieser Sitzung gelesen, `datei:zeile` oben):** alle EmuTOS-Pfade
(`ev_multi`-Quick-Checks, `ap_rdwr`, `aqueue`/`doq`, `signal`/`azombie`/`mwait`/`apret`,
`ap_init`, `ap_exit`, `fpdnm`, `wm_open`/`draw_change`/`w_update`/`w_redraw`/`ap_sendmsg`,
`wm_new`, `deskmain.c:2166`, `gemshlib.c:636,650-657,676-677`); alle NewTOS-Pfade (`evnt_multi`,
`appl_init`/`appl_write`/`appl_read`, `pd::open`/`close`/`register`/`post_msg`, `MsgQueue`,
`wind_open`/`send_redraw`/`wind_new`, `arm_or_get_deadline`, `desktop_relinquish`); der Riegel
und Runner; die Messwerte aus M07 (`int_out[0] = 32` in Fall 69 auf beiden Maschinen; `ap_id`
0/1) und A06 (`ap_id` 2 beim Desktop-Start); `casecheck` Exit 0 für den Entwurf.

**Gefolgert (gekennzeichnet):**
- `evnt_multi(MU_TIMER, 0)` kehrt auf der **echten Pi 4** zurück — aus dem Code
  (`event.rs:455-459,613-619`) plus dem QEMU-Lauf (M07) plus `evnt_timer(0/1)` auf der echten
  Pi über dieselben Primitive (r1:228-229, run1-pi4). Nicht auf der Pi gemessen.
- Der nicht-leere `MU_MESAG`-Zweig kehrt auf EmuTOS ohne Warten zurück — aus
  `gemqueue.c:98-107` + `gemasync.c:40,117-127`. Nicht gemessen; **C1 misst es**, und der
  Hatari-Lauf geht deshalb vor jedem Pi-Lauf.
- `save_windows()` entfernt die Desktop-Fenster vor dem Dumperstart — aus dem A25-Audit
  (`window.rs:805-822`), nicht selbst gelesen. Betrifft nur die Lesart von P1.
- Die Desktop-PID/Slot-Teilung (`current_pid() = 1`, `pid_of_slot(0) = 1`) — aus A25-Audit und
  A28-Status, nicht selbst gelesen. Betrifft nur 1.3.

---

## 7. Nebenbeobachtungen außerhalb des Auftrags (nicht verfolgt)

- **N1 — Riegellücke `appl_write` (12):** `NULL` im Opcode-Riegel (`cases.c:484`), obwohl eine
  `AP_RWID` ohne Deskriptor EmuTOS' `aqueue` durch NULL lesen und `doq` durch `p_qaddr`
  schreiben lässt (`gemqueue.c:94-97,39`); NewTOS antwortet 0. Ein M07-artiger Parameterarm
  fehlt; die r4-Tabelle hält 12 nur per Kommentar fern.
- **N2 — Veraltete Begründungen im Dumper:** `cases.c:1846-1848` („has no non-blocking way to
  drain its own queue") und der `TD_AES_MAX_OPEN_WINDOWS`-Kommentar in `cases.h` sagen, es gebe
  keinen nicht blockierenden Drain — seit M07 gibt es ihn (1.1). Die Schranke selbst bleibt
  richtig, ihre Begründung nicht.
- **N3 — Spec `application.md:225`:** „blocks via `ev_block` … until the queue operation
  completes" lässt den sofortigen Rückweg (`qready` wahr → `azombie` → `mwait` ohne `dsptch`)
  und den Sender-Block auf voller Queue (`gemqueue.c:120-124`; NewTOS 0) unerwähnt. Für einen
  Entwickler, der `appl_write` implementiert, ist Letzteres ein Vertragsunterschied.
- **N4 — `appl_read` NewTOS nicht blockierend** (`appl.rs:366-369`), EmuTOS blockierend: über
  A25 hinaus ein eigener Vertragsunterschied (vom Code selbst als Bring-up-Stand benannt).
