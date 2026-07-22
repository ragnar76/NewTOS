# AUD-P2 — Event / Queue / Dispatch / Control-Manager (1:1-Divergenz-Katalog)

> Stand: 2026-07-22, Branch `desk-rebuild`. Read-only Audit (kein Produkt-Code).
> Scope P2: die Kern-Event-Maschinerie — `evnt_*`, Message-FIFO (enqueue/dequeue
> + WM_REDRAW-Union), Dispatcher, Control-Manager, `gemasync`/`gemflag`-Semaphoren.
> NewTOS-Impl: `tos-api/src/aes/event.rs` (+ Queue-Mechanik in `aes/pd.rs`,
> Routing/Intercept in `aes/window.rs`, Poster in `aes/appl.rs`).
> EmuTOS-Quelle: `gemevlib.c`, `gemdisp.c`, `gemqueue.c`, `geminput.c`,
> `gemctrl.c`, `gemasync.c`, `gemflag.c`, `struct.h`, `gemsuper.c` (Dispatch).
> Soll-Referenz: `docs/api/aes/event.md`, `_dispatcher.md`, `_discrepancies.md`
> D-049/D-050 — **geprüft, korrekt** (Code weicht ab, nicht die Spec).
>
> Architektur-Vorbemerkung: EmuTOS ist ein präemptiv-kooperatives Mini-Multitasking
> über **EVB** (Event-Blocks), einen **Fork-Ring**, per-Prozess **`p_queue`** und
> einen eigenen **`ctlmgr`-Prozess**. NewTOS ersetzt die gesamte Maschinerie durch
> **eine kooperative Poll-Schleife pro `evnt_*`-Trap** (Single-AES-App-Bring-up).
> Die reine Scheduler-Mechanik (EVB/Fork/dsptch) ist bei Single-App unbeobachtbar
> und als HW-/Architektur-Abweichung akzeptabel — **aber** die *sichtbaren*
> Konsequenzen (Message-Routing, Return-ABI, Blocking-Semantik) sind es nicht.
> Dieser Katalog trennt beides.

---

## Teil 1 — Coverage-Tabelle

### gemevlib.c (Event-Library)

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| `ev_rets` | gemevlib.c:42 | `fill_rets` event.rs:95 | partial | **no** (mtrans/pr_* fehlt) |
| `ev_block` | gemevlib.c:64 | — (in Poll-Loops inline) event.rs:131,220,459 | MISSING (als Fn) | n/a |
| `ev_button` | gemevlib.c:84 | `evnt_button` event.rs:123 | partial | **no** (bflag) |
| `ev_mesag` | gemevlib.c:100 | `evnt_mesag` event.rs:399 | partial | **no** (p_msg-Fastpath) |
| `ev_mouse` | gemevlib.c:123 | `evnt_mouse` event.rs:211 | impl | yes |
| `ev_timer` | gemevlib.c:134 | `evnt_timer` event.rs:438 | impl | ~ (Tick-Quant.) |
| `ev_multi` | gemevlib.c:152 | `evnt_multi` event.rs:260 | partial | **no** (Bypass/bflag/rets) |
| `ev_dclick` | gemevlib.c:300 | `evnt_dclick` event.rs:234 | impl | yes |
| `evnt_keybd` (=`ev_block(MU_KEYBD)`) | gemsuper.c:127 | `evnt_keybd` event.rs:376 | impl | yes |

### gemqueue.c (Message-FIFO)

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| `doq` (enqueue-Hälfte) | gemqueue.c:31 | `MsgQueue::push` pd.rs:96 | partial | **no** (Byte-Arith/`om[2]`) |
| `doq` (dequeue-Hälfte) | gemqueue.c:76 | `MsgQueue::pop` pd.rs:123 | impl | yes |
| `doq` WM_REDRAW-Union | gemqueue.c:45-56 | `rc_union_into`/`push` pd.rs:98-110,144 | impl | yes |
| `aqueue` | gemqueue.c:85 | `post_msg`/`push` pd.rs:233,96 | partial | **no** (kein Block/Wakeup) |

### gemdisp.c (Dispatcher / Fork-Ring)

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| `forkq` | gemdisp.c:52 | — | MISSING | n/a (akzeptabel) |
| `forker` | gemdisp.c:101 | — | MISSING | n/a (akzeptabel) |
| `chkkbd` | gemdisp.c:158 | `console_try_read_key`-Poll event.rs:305,379 | partial | ~ (kein per-PD c_q) |
| `disp` / `schedule` / `disp_act` / `mwait_act` | gemdisp.c:221/181/74/82 | — | MISSING | n/a (akzeptabel) |

### geminput.c (Input-Async / Button-Click / Kbd-Ring)

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| `in_mrect` | geminput.c:76 | `rect_event` event.rs:42 | impl | yes |
| `chk_ctrl` | geminput.c:87 | `find_handle`-basiert window.rs:1812 | partial | ~ |
| `downorup` | geminput.c:348 | — (Gleichheit statt Prädikat) event.rs:129,311 | MISSING | **no** (bflag) |
| `b_click` / `b_delay` | geminput.c:114/155 | `count_clicks` (Poll) event.rs:183 | partial | ~ |
| `bchange` / `mchange` | geminput.c:328/360 | — | MISSING | n/a |
| `kchange` / `post_keybd` | geminput.c:288/299 | — | MISSING | n/a |
| `nq` / `dq` / `fq` | geminput.c:241/256/272 | — (Host-Key-Ring) | MISSING | ~ |
| `akbin` / `adelay` / `abutton` / `amouse` | geminput.c:486/496/545/568 | — | MISSING | n/a (EVB-arm) |
| `post_mouse` / `post_button` / `inorout` | geminput.c:446/463/427 | — | MISSING | n/a |
| `evremove` | geminput.c:279 | — | MISSING | n/a |
| `set_ctrl`/`get_ctrl`/`set_mown`/`get_mown` | geminput.c:189/200/220/209 | — | MISSING | n/a (akzeptabel) |

### gemctrl.c (Control-Manager)

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| `ctlmgr` (eigener Prozess) | gemctrl.c:427 | — (in `evnt_multi` gefaltet) event.rs:296-303 | MISSING (als Prozess) | **no** |
| `ct_msgup` | gemctrl.c:81 | `emit` window.rs:1880 | partial | **no** (Queue-Bypass) |
| `hctl_window` | gemctrl.c:158 | `window::intercept` window.rs:1807 | partial | **no** (Bypass) |
| `hctl_rect` | gemctrl.c:318 | `menu_intercept` event.rs:58 | partial | **no** (Bypass) |
| `handle_arrow_msg` | gemctrl.c:111 | `intercept` Arrow-Branch window.rs:1865 | partial | **no** (kein Continuous) |
| `perform_untop` | gemctrl.c:146 | — | MISSING | **no** (WM_UNTOPPED fehlt) |
| `hctl_msg` | gemctrl.c:380 | — | MISSING | ~ (3D-only) |
| `ct_chgown` / `ct_mouse` | gemctrl.c:392/400 | — | MISSING | n/a (akzeptabel) |

### gemasync.c / gemflag.c (Semaphoren / Timer-Tick)

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| `iasync` / `mwait` / `apret` / `acancel` | gemasync.c:130/117/182/217 | — | MISSING | n/a (akzeptabel) |
| `azombie` / `evinsert` / `takeoff` / `signal` | gemasync.c:64/81/96/35 | — | MISSING | n/a (akzeptabel) |
| `tchange` (Timer-Tick) | gemflag.c:32 | `monotonic_millis`-Poll event.rs:450 | partial | ~ |
| `tak_flag` / `amutex` / `unsync` (MU_MUTEX) | gemflag.c:71/89/105 | — (`wind_update` No-Op) window.rs:1461 | MISSING | ~ (akzeptabel, single-app) |

### Message-Send-Pfad (gemwmlib.c / gemaplib.c)

| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| `ap_sendmsg` | gemwmlib.c:851 | `send_redraw` (Queue) window.rs:628 / `emit` (Bypass) window.rs:1880 | partial | **no** (Bypass-Zweig) |
| `ap_rdwr` | gemaplib.c:69 | `appl_read`/`appl_write`/`evnt_mesag`-pop appl.rs:317,351 / event.rs:399 | partial | **no** (nicht-blockend) |
| `w_redraw` | gemwmlib.c:887 | `send_redraw_in`/`send_redraw` window.rs:685,628 | impl | yes |

---

## Teil 2 — Findings

### AUD-P2-001: `ctlmgr`/`ct_msgup` — Fenster-Rahmen-Messages umgehen die per-App-FIFO (Queue-Bypass, Drop + Reorder + Spurious-MESAG)

- **Severity: BLOCKER**
- **EmuTOS:** `gemctrl.c:81-102` (`ct_msgup` → `ap_sendmsg`), `gemwmlib.c:851-863`
  (`ap_sendmsg` → `ap_rdwr(MU_SDMSG)`), `gemaplib.c:69-89` (`ap_rdwr` →
  `ev_block(MU_SDMSG)`), `gemasync.c:170-171`/`gemqueue.c:85-125` (`aqueue`/`doq`
  → append in `p_queue` des **Fenster-Owners**), `gemevlib.c:100-117` +
  `gemsuper.c:135-141` (Empfänger liest via `ev_mesag`/`ev_multi` → `doq`-dequeue).
  ALLE ctlmgr-Messages (WM_CLOSED/FULLED/MOVED/SIZED/TOPPED/ARROWED/UNTOPPED)
  laufen durch **genau diesen einen Pfad** in die FIFO und werden dort FIFO-korrekt
  eingereiht (bzw. WM_REDRAW/ARROWED koalesziert, `gemqueue.c:45-67`).
- **NewTOS:** `window.rs:1807-1878` (`intercept`) baut die WM_*-Message inline und
  schreibt sie via `emit` (`window.rs:1880-1891`) **direkt in `pbuff`**
  (`= MME_PBUFF`, `addr_in[0]`), ohne `post_msg`/`queue.push`. Aufgerufen aus
  `evnt_multi` (`event.rs:300`), **bevor** im selben Schleifendurchlauf der
  `MU_MESAG`-Zweig (`event.rs:331-351`) `queue.pop()` macht und `pbuff`
  **überschreibt**.
- **Divergenz:**
  1. **Drop:** Liegt beim Rahmen-Klick bereits eine Message in der FIFO (z. B. ein
     `WM_REDRAW` aus einer vorherigen Fensteroperation), überschreibt der
     `queue.pop()` in `event.rs:337-349` die von `intercept` in `pbuff` gelegte
     `WM_CLOSED` mit dem gepoppten `WM_REDRAW` — `fired` bleibt `MU_MESAG`, die App
     sieht das `WM_REDRAW`, das `WM_CLOSED` ist **für immer verloren** (es war nie
     in der Queue). Ergebnis: das angeklickte Fenster **schließt nicht**. Das ist
     dieselbe Fehlerfamilie wie die offenen Redraw-Bugs #127.
  2. **Reorder/FIFO-Bruch:** Rahmen-Messages überholen bzw. verdrängen bereits
     gequeuete Messages statt hinter ihnen einzureihen (`doq` hängt an; NewTOS
     jumpt/clobbert).
  3. **Falsche Owner-Zustellung (latent):** `intercept` schreibt immer in `pbuff`
     des *laufenden* Prozesses (`current_pid`), nicht in `p_queue` des
     `w_owner`-Prozesses. Bei Multi-App (Desktop führt ctlmgr-Loop, App besitzt das
     Fenster) ginge die Message an den falschen Prozess. Bei Single-App latent.
- **1:1-Fix:** `intercept`/`emit` müssen die Message via `pd::post_msg(w_owner, msg)`
  in die FIFO des Fenster-Owners legen (Pfad wie `send_redraw`, `window.rs:628-649`),
  **nicht** in `pbuff`. `evnt_multi` darf `pbuff` ausschließlich aus `queue.pop()`
  füllen. Damit greifen automatisch FIFO-Ordnung und WM_REDRAW/ARROWED-Koaleszenz.
- **Runtime-verify: yes** (Repro: Fenster öffnen → WM_REDRAW steht in Queue →
  Closer-Gadget klicken → Fenster bleibt offen).

---

### AUD-P2-002: `hctl_rect` — Menü-Selektion (`MN_SELECTED`) umgeht die FIFO + `msg[1]`-Sender-PID = 0 (Queue-Bypass)

- **Severity: BLOCKER**
- **EmuTOS:** `gemctrl.c:318-371` (`hctl_rect` → `ct_msgup(MN_SELECTED, gl_mnppd,
  title, item, …)`) → `ap_sendmsg` (`gemwmlib.c:851`) setzt `msg[1] = rlr->p_pid`
  und postet via `aqueue`/`doq` in `p_queue` des Menü-Owners.
- **NewTOS:** `event.rs:58-90` (`menu_intercept`) schreibt `MN_SELECTED` mit
  `msg[3]=title`, `msg[4]=item` **direkt in `pbuff`** (`event.rs:77-84`), ohne
  Queue; `msg[1]` (Sender-PID) bleibt **0** (nur `msg[0]/msg[3]/msg[4]` gesetzt).
  Aufgerufen aus `evnt_multi` (`event.rs:297`), gefolgt vom potenziell
  überschreibenden `queue.pop()` (`event.rs:337`).
- **Divergenz:** (a) selber Bypass/Drop-Mechanismus wie AUD-P2-001 — eine gequeuete
  Message klobbert die frisch geschriebene `MN_SELECTED`; (b) `msg[1]` muss die
  Sender-`p_pid` tragen (EmuTOS `ap_sendmsg`), NewTOS liefert 0 (keine gültige PID);
  (c) bei `CONF_WITH_MENU_EXTENSION` fehlen zusätzlich `msg[5..7]` (treehi/treelo/
  parent, `gemctrl.c:356-359`).
- **1:1-Fix:** `menu_intercept` muss `MN_SELECTED` via `pd::post_msg(owner, msg)`
  in die FIFO legen (mit `msg[1]=current_pid`), nicht in `pbuff`.
- **Runtime-verify: yes** (Menüpunkt wählen während eine Message gequeuet ist →
  `MN_SELECTED` geht verloren; und `msg[1]==0` prüfbar per Selbsttest).

---

### AUD-P2-003: `evnt_multi` — `MU_MESAG` wird zurückgemeldet, obwohl die App nur `MU_BUTTON` angefordert hat (falsches Return-Bit + Fremdschreib in `pbuff`)

- **Severity: BLOCKER**
- **EmuTOS:** Rahmen-Klicks werden vom **ctlmgr-Prozess** verarbeitet (`gemctrl.c:427`),
  der den Mauszeiger über Rahmen/Menü besitzt (`chk_ctrl` → `ctl_pd`,
  `geminput.c:87-107,316-325`). Eine App, die in `ev_button`/`ev_multi` **ohne**
  `MU_MESAG` wartet, wird durch einen Rahmen-Klick **nicht** geweckt und bekommt
  **kein** `MU_MESAG`-Bit; die Message liegt still in ihrer `p_queue`, bis sie
  später `MU_MESAG` anfordert (`gemsuper.c:147-148` setzt `AP_MESAG` nur bei
  `MU_MESAG`).
- **NewTOS:** `event.rs:296` — `if flags & (MU_BUTTON | MU_MESAG) != 0` ruft
  `intercept`/`menu_intercept` **auch dann**, wenn nur `MU_BUTTON` gesetzt ist, und
  setzt bei Erfolg `fired |= MU_MESAG` (`event.rs:301`), obwohl die App `MU_MESAG`
  nie angefordert hat. Zusätzlich schreibt `emit`/`menu_intercept` in `pbuff =
  addr_in[0]`, das eine App ohne `MU_MESAG` gar nicht als gültigen Buffer gesetzt
  haben muss → Schreib in einen Stale-Pointer (Speicherkorruption) bzw. die App
  liest anschließend einen uninitialisierten Buffer als „gültige Message".
- **Divergenz:** Falsches Return-Bit (`MU_MESAG` ohne Anforderung = ABI-Verstoß) +
  potenzieller Fremdschreib/Garbage-Read.
- **1:1-Fix:** Rahmen-/Menü-Interception darf nur `pbuff` befüllen bzw. `MU_MESAG`
  setzen, **wenn `flags & MU_MESAG`** gesetzt ist; ansonsten (nur `MU_BUTTON`) darf
  sie den Klick nur als `MU_BUTTON`-Event über die reguläre Button-Logik melden.
  Sauber gelöst durch AUD-P2-001 (Message in die FIFO), plus Guard `flags &
  MU_MESAG` vor dem Intercept.
- **Runtime-verify: yes** (Port-App ruft `evnt_multi(MU_BUTTON, …, addr_in[0]=0)`,
  Rahmen-Klick → Rückgabe enthält fälschlich `MU_MESAG`).

---

### AUD-P2-004: `ev_button`/`ev_multi` MU_BUTTON — `bflag` (enter/leave) und `downorup`-Prädikat fehlen; `clicks`-High-Bit falsch interpretiert

- **Severity: HIGH**
- **EmuTOS:** `gemevlib.c:84-94` (`ev_button` → `combine_cms(bflgclks,bmask,bstate)`,
  `gemevlib.h:34-50`) packt `clicks:WORD ∥ mask:byte ∥ state:byte`; `downorup`
  (`geminput.c:348-357`) wertet `flag = (buparm>>24)&0xff` (High-Byte des
  clicks-WORD) aus: `return (((mask & (val^new)) == 0) != flag)`. `flag=0` ⇒ warten
  bis Zustand **erreicht**, `flag=1` (High-Bit von `clicks`) ⇒ warten bis Zustand
  **verlassen**. `docs/api/aes/event.md:427-431,506-507` dokumentiert das korrekt.
- **NewTOS:** `evnt_button` (`event.rs:123-141`) und `evnt_multi` (`event.rs:310-323`)
  nutzen als Prädikat nur `mouse_buttons() & mask == state & mask` (`event.rs:129,311`)
  — der **enter**-Fall ohne `flag`. Das `clicks`-Wort wird als roher Zählwert an
  `count_clicks(max_clicks, …)` durchgereicht (`event.rs:137,320`), inklusive des
  `flag`-High-Bits.
- **Divergenz:**
  1. **Kein Leave-Wait:** Eine App, die auf Button-**Loslassen** wartet
     (`flag=1`, z. B. `bflgclks & 0x8000`), bekommt in NewTOS das Enter-Verhalten —
     falsches Event.
  2. **Zählwert-Verfälschung:** Ist das High-Bit gesetzt, wird `intin[1]` als `i16`
     negativ (z. B. `0x8001 → -32767`); `count_clicks`' `while got < max_clicks`
     terminiert sofort mit `got=1`. Bei sauberem `flag=0`-Aufruf (Desktop:
     `buparm=0x0001ff01`, `clicks=1`) unschädlich — aber ABI-falsch für konforme
     Apps, die den `flag`-Bit nutzen.
- **1:1-Fix:** `downorup(new, buparm)` 1:1 implementieren (`flag`/`mask`/`val`
  aus dem gepackten LONG) und als Warte-Prädikat verwenden; die Klickzahl aus dem
  **Low-WORD** von `clicks` (bzw. `combine_cms`-Layout) nehmen, nicht aus dem
  Rohwort.
- **Runtime-verify: yes**

---

### AUD-P2-005: `handle_arrow_msg` — `WM_ARROWED` nur einmal statt kontinuierlich bei gehaltenem Scroll-Pfeil; `ev_mesag` `p_msg`-Fastpath fehlt

- **Severity: HIGH**
- **EmuTOS:** `gemctrl.c:111-139` (`handle_arrow_msg`) sendet `WM_ARROWED`
  **wiederholt** in einer `do { … } while(button & 0x0001)`-Schleife, solange der
  linke Button gedrückt ist — direkt via `ap_sendmsg` wenn der Owner wartet, sonst
  über `p_msg.action`/`p_msg.wh` (`gemctrl.c:123-134`). Der Empfänger liest den
  `p_msg`-Fastpath in `ev_mesag` (`gemevlib.c:102-114`: `p_qindex==0 &&
  p_msg.action>=0` ⇒ synthetisiert `WM_ARROWED`) bzw. im `ev_multi`-Quick-Check
  (`gemevlib.c:224`). `docs/api/aes/event.md:274-279,440-442` dokumentiert das.
- **NewTOS:** `window.rs:1865-1874` (`intercept` Arrow-Branch) baut **genau eine**
  `WM_ARROWED`-Message und ruft `wait_release()` (wartet auf Loslassen) — also
  ein Scroll-Schritt pro Klick. Der `p_msg`-Mechanismus existiert nicht (kein
  `p_msg`-Feld im `Aespd`, `pd.rs:156-164`), `evnt_mesag`/`evnt_multi` prüfen nur
  `queue.pop()` (`event.rs:416,337`), nicht `p_msg.action`.
- **Divergenz:** Gehaltener Scroll-Pfeil scrollt in NewTOS **einmal**, in EmuTOS
  **kontinuierlich** (Auto-Repeat) — sichtbares Bedienverhalten.
- **1:1-Fix:** Arrow-Branch als `do { post WM_ARROWED to owner; dsptch } while(button)`
  nachbilden (bzw. den `p_msg`-Fastpath + kontinuierliches Posten in die FIFO
  implementieren), statt Single-Shot + `wait_release`.
- **Runtime-verify: yes**

---

### AUD-P2-006: `ev_rets` — Rückgabeslots liefern immer die Live-Mausposition statt des Transition-Snapshots (`mtrans`/`pr_xrat`/`pr_yrat`/`pr_button`)

- **Severity: HIGH**
- **EmuTOS:** `gemevlib.c:42-58` (`ev_rets`): bei anstehendem Button-Übergang
  (`mtrans > 1`) werden `rets[0..2] = pr_xrat/pr_yrat/pr_button` (Zustand **zum
  Zeitpunkt des Klicks**) geliefert, sonst die Live-Werte `xrat/yrat/button`; danach
  `mtrans = 0`. `docs/api/aes/event.md:187-188,434` dokumentiert das korrekt.
- **NewTOS:** `fill_rets` (`event.rs:95-101`) setzt `EV_MX/EV_MY/EV_MB` **immer** aus
  `vdi::mouse_position()`/`mouse_buttons()` (Live-Zustand); kein `mtrans`/`pr_*`.
  In `evnt_multi` wird `fill_rets` erst **nach** `count_clicks` gerufen
  (`event.rs:363`), das bei Doppelklick-Wait bis zu einem vollen Doppelklick-Fenster
  (~220 ms) wartet.
- **Divergenz:** `EV_MX`/`EV_MY` melden die Mausposition **nach** dem Ereignis, nicht
  die **Klickposition**. Bewegt sich die Maus zwischen Buttondown und dem
  `fill_rets`-Read (insb. Drag oder während des Doppelklick-Fensters), zeigt der
  Desktop-`wm_find(rets[0],rets[1])` auf das falsche Fenster/Icon.
- **1:1-Fix:** `pr_xrat`/`pr_yrat`/`pr_button` + `mtrans`-Mirror einführen; beim
  Button-Übergang die Klickposition festhalten und `ev_rets`-Logik (`mtrans>1` ⇒
  `pr_*`) 1:1 nachbilden.
- **Runtime-verify: yes**

---

### AUD-P2-007: `evnt_mesag`/`evnt_multi` — `AP_MESAG` nicht unbedingt vor dem Warten gesetzt; `AC_CLOSE → AP_ACCLOSE` fehlt

- **Severity: MEDIUM**
- **EmuTOS:** `gemsuper.c:137` setzt `rlr->p_flags |= AP_MESAG` **vor** `ev_mesag`
  (bzw. `gemsuper.c:147-148` bei `evnt_multi`, wenn `MU_MESAG`), unabhängig davon ob
  eine Message ankommt; nach dem Lesen `if (msg[0]==AC_CLOSE) p_flags |= AP_ACCLOSE`
  (`gemsuper.c:139-140,160-161`). `AP_ACCLOSE` steuert `ap_exit`'s
  `wait_for_accs(AP_ACCLOSE)` (`gemaplib.c:255`).
- **NewTOS:** `AP_MESAG` wird nur **innerhalb** des `queue.pop()`-Zweigs gesetzt
  (`event.rs:347,426`), also nur wenn tatsächlich eine Message geliefert wurde;
  `AP_ACCLOSE` existiert nicht (`pd.rs:22-25` definiert nur `AP_OPEN`/`AP_MESAG`).
- **Divergenz:** `AP_MESAG`-Timing weicht ab; DA-`AC_CLOSE`-Buchhaltung fehlt.
  Bei Single-App-Desktop ohne echte DAs weitgehend unbeobachtbar, aber ein
  Verstoß gegen den 1:1-Kontrakt und relevant, sobald Accessories laufen.
- **1:1-Fix:** `AP_MESAG` beim Eintritt in `evnt_mesag`/`evnt_multi(MU_MESAG)` setzen;
  `AP_ACCLOSE`-Flag ergänzen und bei `msg[0]==AC_CLOSE` setzen.
- **Runtime-verify: no** (statisch belegbar; Beobachtung erst mit DA-Pfad)

---

### AUD-P2-008: `evnt_mesag` — Frühausstieg meldet „Message ready" (RET=1) ohne Message zu schreiben

- **Severity: MEDIUM**
- **EmuTOS:** `ev_mesag` (`gemevlib.c:100-117`) blockiert immer, bis eine echte
  16-Byte-Message vorliegt; `rlr` existiert für jeden AES-Prozess.
- **NewTOS:** `event.rs:402-404` (`host()==None`) und `event.rs:408-413`
  (`by_pid(pid)==None`, Aufrufer ohne `appl_init`) setzen **`ret=1`** und kehren
  zurück, **ohne** in `dest` zu schreiben. Die App liest ihren uninitialisierten
  Message-Buffer als gültige Message.
- **Divergenz:** Falscher Erfolg (RET=1) ohne Message → Garbage-Read/Fehlverhalten.
- **1:1-Fix:** Ohne gültigen Empfänger nicht mit RET=1 „Erfolg" simulieren; entweder
  Fehler melden oder (wie EmuTOS) einen validen Descriptor voraussetzen. Der
  `host()==None`-Zweig ist reiner Test/Bring-up-Pfad — mindestens `dest`
  nullen, nicht als „Message" ausgeben.
- **Runtime-verify: no** (statisch)

---

### AUD-P2-009: `aqueue`/`doq` — kein Sender-Blocking bei voller Queue; Message wird still verworfen

- **Severity: MEDIUM**
- **EmuTOS:** `aqueue` (`gemqueue.c:96-124`): passt die Message nicht in `p_queue`
  (`qready==0`), wird der **Sender blockiert** (`e->e_parm=lm; evinsert(e,&p_qnq)`)
  und beim nächsten Dequeue geweckt (`gemqueue.c:108-118`). Kein Message-Verlust.
- **NewTOS:** `MsgQueue::push` (`pd.rs:112-114`) gibt bei `count >= QUEUE_MSGS`
  (=8) `false` zurück; `post_msg` (`pd.rs:233-239`) und die Aufrufer
  (`send_redraw` `window.rs:648` ignoriert den Rückgabewert; `appl_write`
  `appl.rs:340` meldet 0) **verwerfen die Message ersatzlos**.
- **Divergenz:** Bei Überlauf geht die Message verloren statt den Sender zu parken.
  Durch Same-Window-Redraw-Koaleszenz + per-Loop-Drain im Desktop praktisch
  schwer erreichbar; adversarial (≥8 Messages nicht-koaleszierender Typen, oder
  9 distinkte Fenster-Redraws) aber real.
- **1:1-Fix:** Bei Single-App-Bring-up ohne Scheduler ist echtes Sender-Blocking
  nicht 1:1 möglich; der Kontrakt „kein stiller Drop" ist dennoch einzuhalten —
  z. B. Queue-Kapazität ≥ EmuTOS (`QUEUE_SIZE`-Bytes-Semantik) und der `send_redraw`-
  Rückgabewert darf nicht ignoriert werden. Als bewusste Abweichung dokumentieren.
- **Runtime-verify: yes** (8+ Nicht-Redraw-Messages posten → 9. verschwindet)

---

### AUD-P2-010: `doq` — Queue in ganzen 16-Byte-Slots statt Byte-`p_qindex`; variable Länge (`om[2]`) und Nicht-16-Byte-Messages nicht unterstützt

- **Severity: MEDIUM**
- **EmuTOS:** `doq` (`gemqueue.c:31-82`) arbeitet **byteweise** auf `p_queue`
  (`p_qindex` in Bytes); der Union-Scan überspringt Einträge mit
  `index += om[2] + 16` (`gemqueue.c:69`), d. h. `msg[2]` = zusätzliche Bytes über
  die 16 hinaus. `ap_rdwr`/`appl_write` können Längen ≠ 16 transportieren
  (`AP_LENGTH`, `gemsuper.c:104`).
- **NewTOS:** `MsgQueue` (`pd.rs:52-56`) speichert `[[i16;8]; 8]` — **feste**
  8-WORD-Slots, `count` in ganzen Messages. Der Union-Scan (`pd.rs:99-110`) läuft
  über Message-Indizes ohne `om[2]`-Längenlogik. `appl_write` (`appl.rs:330`) und
  `appl_read` (`appl.rs:370`) truncaten auf `MSG_WORDS`.
- **Divergenz:** Messages ≠ 16 Byte (variable-Length, `msg[2] != 0`) sind nicht
  darstellbar; die `om[2]`-Skip-Arithmetik des Union-Scans divergiert. Alle
  Standard-GEM-Messages sind 16 Byte ⇒ praktisch selten, aber `appl_write` mit
  `len != 16` bzw. Extended-Messages weichen ab.
- **1:1-Fix:** Byte-basiertes `p_queue`/`p_qindex`-Modell mit `om[2]`-Längenfeld
  nachbilden (oder die feste 16-Byte-Annahme explizit im Kontrakt festschreiben und
  `appl_write` mit `len != 16` ablehnen statt zu truncaten).
- **Runtime-verify: no** (statisch)

---

### AUD-P2-011: `appl_read` — nicht-blockend (liefert 0 bei leerer Pipe) statt zu blockieren

- **Severity: MEDIUM**
- **EmuTOS:** `APPL_READ` → `ap_rdwr(MU_MESAG, …)` (`gemsuper.c:101-104`) →
  `ev_block(MU_MESAG)` (`gemaplib.c:88`): **blockiert**, bis eine Message vorliegt,
  gibt dann 1 zurück.
- **NewTOS:** `appl_read` (`appl.rs:351-380`) gibt bei leerer Queue sofort **0**
  zurück (`appl.rs:366-369`) — nicht-blockend.
- **Divergenz:** Eine App, die `appl_read` als blockierendes Rendezvous nutzt,
  bekommt stattdessen sofort 0. (Aufrufer, die `evnt_mesag` verwenden, sind nicht
  betroffen — nur der direkte `appl_read`-Pfad.)
- **1:1-Fix:** `appl_read` wie `evnt_mesag` als Poll-Loop blockieren lassen
  (bis `queue.pop()` liefert), statt 0-Frühausstieg.
- **Runtime-verify: no** (statisch; `appl_read` ist P1-nah, hier wegen Queue-Pfad
  gelistet)

---

### AUD-P2-012: `perform_untop`/`WM_UNTOPPED` — beim Antippen eines inaktiven Fensters wird kein `WM_UNTOPPED` an das bisher oberste Fenster gesendet

- **Severity: MEDIUM**
- **EmuTOS:** `hctl_window` (`gemctrl.c:172-180`) ruft bei Klick auf ein nicht-oberstes
  Fenster zuerst `perform_untop(gl_wtop)` (`gemctrl.c:146-155` → `ct_msgup(WM_UNTOPPED,
  …)`) und dann `ct_msgup(WM_TOPPED, …)` für das neue.
- **NewTOS:** `intercept` (`window.rs:1824-1830`) sendet bei `handle != get_top()` nur
  `WM_TOPPED`; kein `WM_UNTOPPED` an das alte Top-Fenster.
- **Divergenz:** Das bisher oberste Fenster erhält kein `WM_UNTOPPED` — Apps, die
  darauf ihren Fokus/Highlight zurücknehmen, bleiben „aktiv" dargestellt.
- **1:1-Fix:** Vor `WM_TOPPED` ein `WM_UNTOPPED` an den Owner des bisherigen
  Top-Fensters posten (in dessen FIFO, s. AUD-P2-001).
- **Runtime-verify: yes**

---

### AUD-P2-013: `ev_multi` MU_TIMER — keine `gl_ticktime`-Quantisierung (raw ms)

- **Severity: LOW**
- **EmuTOS:** `ev_timer`/`ev_multi` (`gemevlib.c:136,259`) rechnen
  `tmcount / gl_ticktime` und zählen in **Ticks** (`adelay`/`tchange`); sub-Tick-Zeiten
  (< `gl_ticktime` ms) fallen auf 0 und feuern sofort.
- **NewTOS:** `evnt_timer`/`evnt_multi` (`event.rs:441,282`) rechnen roh in ms gegen
  `monotonic_millis()` — höhere Auflösung, keine Tick-Quantisierung.
- **Divergenz:** Ein 5-ms-Timer feuert in EmuTOS (bei z. B. 20 ms/Tick) sofort, in
  NewTOS erst nach 5 ms. Sub-Tick-Timing; für Apps unbeobachtbar bis auf exakte
  Zeitmessung. Als **akzeptable HW-Timer-Abweichung** einzustufen — dennoch gelistet.
- **1:1-Fix:** Optional; falls exakte TOS-Parität gewünscht, auf `gl_ticktime`
  quantisieren. Sonst als bewusste Abweichung dokumentieren.
- **Runtime-verify: no**

---

### AUD-P2-014: WM_*-/Redraw-Message `msg[1]` = Empfänger-PID statt Sender-PID; ctlmgr-Herkunft nicht abgebildet

- **Severity: LOW**
- **EmuTOS:** `ap_sendmsg` (`gemwmlib.c:855`) setzt `msg[1] = rlr->p_pid` = PID des
  **Senders**. Für ctlmgr-Messages (WM_CLOSED etc.) ist das die `ctl_pd`-PID; für
  `w_redraw` (vom App-Kontext) die App-PID.
- **NewTOS:** `send_redraw` (`window.rs:635`) und `intercept` (`window.rs:1821`)
  setzen `msg[1] = current_pid()` (= laufender/Empfänger-Prozess); `menu_intercept`
  setzt `msg[1] = 0` (s. AUD-P2-002).
- **Divergenz:** Für `w_redraw` deckungsgleich (Sender=App). Für ctlmgr-Messages
  weicht `msg[1]` ab (EmuTOS: ctlmgr-PID). Apps lesen `msg[1]` selten aus; LOW.
- **1:1-Fix:** Konsistente Sender-PID setzen (bei Single-App = App-PID akzeptabel;
  `menu_intercept`s `0` ist zu korrigieren).
- **Runtime-verify: no**

---

### AUD-P2-015: `gemflag`-Semaphoren (`tak_flag`/`amutex`/`unsync`, MU_MUTEX) fehlen; `wind_update` ist No-Op

- **Severity: LOW (akzeptable Single-App-Abweichung — katalogisiert)**
- **EmuTOS:** `wm_update(BEG/END_UPDATE)` nimmt/gibt eine Screen-Semaphore
  (`gemflag.c:71-139`, `iasync(MU_MUTEX)` → `amutex`, `gemasync.c:173`), die den
  Screen gegen konkurrierende AES-Prozesse sperrt.
- **NewTOS:** `wind_update` (`window.rs:1461-1463`) ist ein `set_ret(1)`-No-Op; keine
  Semaphore, kein MU_MUTEX-Pfad.
- **Divergenz:** Bei Single-AES-App gibt es keinen Konkurrenten ⇒ die Sperre ist
  **unbeobachtbar**; korrekt als HW-/Architektur-Abweichung eingestuft (kein Bug),
  aber der No-Op ignoriert `WM_BEGUP` (Argument) und ist bei Multitasking nachzurüsten.
- **1:1-Fix:** Erst bei AES-Multitasking relevant; dann `tak_flag`/`unsync` auf einer
  Screen-SPB implementieren. Bis dahin bewusst dokumentierter No-Op.
- **Runtime-verify: no**

---

### AUD-P2-016: Dispatcher/EVB-Maschinerie (`iasync`/`mwait`/`apret`/`acancel`/`forkq`/`forker`/`disp`/`schedule`) vollständig abwesend

- **Severity: LOW (akzeptable Architektur-Abweichung — katalogisiert)**
- **EmuTOS:** `gemasync.c`/`gemdisp.c` — kooperativer AES-Scheduler mit EVB-Listen,
  Fork-Ring, `disp`/`switchto`.
- **NewTOS:** Ersetzt durch eine Poll-Schleife pro `evnt_*`-Trap (`event.rs`), die
  die Event-Quellen direkt abfragt.
- **Divergenz:** Die Scheduler-Interna sind bei Single-App unbeobachtbar. **Aber**
  ihre sichtbaren Effekte sind es nicht — genau diese sind oben als eigene Findings
  (AUD-P2-001..009) erfasst. Hier nur zur Vollständigkeit der Präsenz-Prüfung.
- **1:1-Fix:** Kein direkter Fix; die Poll-Schleife muss die *beobachtbaren*
  Semantiken (FIFO-Routing, Return-ABI, Blocking) erfüllen (s. o.).
- **Runtime-verify: no**

---

## Zusammenfassung

**Counts je Severity:** BLOCKER 3 · HIGH 3 · MEDIUM 6 · LOW 4 — 16 Findings über
~9 auditierte Public-Event-Funktionen (+ Queue/ctlmgr/Semaphor-Mechanik).

**Bug-Dichte:** 6 klar sichtbare Bugs (BLOCKER+HIGH) / 9 `evnt_*`-Funktionen; plus
Queue-/Routing-Mechanik. Die restlichen 10 sind MEDIUM/LOW oder als akzeptable
Single-App-/HW-Abweichung eingestuft (explizit gekennzeichnet, nicht als „benign"
weggeredet).

**Top-3 Blocker:**
1. **AUD-P2-001** — `intercept`/`emit` schreibt Fenster-Rahmen-WM_* direkt in
   `pbuff` statt in die per-App-FIFO ⇒ Message-Drop (Fenster schließt nicht),
   FIFO-Reorder. Kern der Redraw-Bug-Familie #127.
2. **AUD-P2-002** — `menu_intercept` schreibt `MN_SELECTED` direkt in `pbuff`
   (+ `msg[1]=0`) ⇒ selber Drop-Mechanismus, verlorene Menü-Selektionen.
3. **AUD-P2-003** — `evnt_multi` meldet `MU_MESAG` zurück, obwohl die App nur
   `MU_BUTTON` angefordert hat, und schreibt dabei in einen evtl. ungültigen
   `pbuff` ⇒ falsches Return-Bit + möglicher Fremdschreib.

**Vollständige Liste ALLER gefundenen Queue-Bypässe** (Message an der FIFO vorbei):
- **B1:** `window.rs:1880-1891` (`emit`, aufgerufen aus `window.rs:1807`
  `intercept`) — WM_CLOSED/FULLED/MOVED/SIZED/TOPPED/ARROWED direkt in `pbuff`
  (AUD-P2-001).
- **B2:** `event.rs:77-84` (`menu_intercept`) — `MN_SELECTED` direkt in `pbuff`
  (AUD-P2-002).
- Kein weiterer Bypass gefunden: der Redraw-Pfad (`send_redraw`→`post_msg`→
  `queue.push`, `window.rs:628-649`), `appl_write` (`appl.rs:340`) und die
  Dequeue-Seite (`evnt_mesag`/`appl_read`→`queue.pop`) laufen korrekt über die FIFO.
  (Der einzige *legitime* EmuTOS-Bypass — der `p_msg.action`-Arrow-Fastpath,
  `gemevlib.c:102-114` — ist in NewTOS gar nicht vorhanden; s. AUD-P2-005.)

**Spec-Status:** `docs/api/aes/event.md` und `_discrepancies.md` D-049/D-050 sind
gegen die Quelle **korrekt** (sie dokumentieren `bflag`/`downorup`, `mtrans`/`pr_*`,
den `p_msg`-Fastpath und den ABI-Slot-Map richtig). Kein Spec-Finding — die
Abweichungen liegen ausschließlich im **Code** (event.rs/window.rs/pd.rs), der
hinter der eigenen, korrekten Spec zurückbleibt.

**Getrieben vs. offen:** Statisch Zeile-für-Zeile getrieben (ABI-Slots gegen
`aesdefs.h` verifiziert, Kontrollfluss/Return/Blocking/Message-Layout verglichen,
Bypass adversarial hergeleitet inkl. konkretem Drop-Szenario). QEMU/Runtime laut
Auftrag nicht ausgeführt (5 parallele Auditoren) — Repro-Items mit
`Runtime-verify: yes` markiert (001, 002, 003, 004, 005, 006, 009, 012).

**Out-of-scope-Beobachtung:** `menu.rs:861-864` schreibt eine `MN_SELECTED`-artige
Struktur direkt in einen Out-Buffer (Menü-Popup-Pfad) — eigener Schreibpfad,
gehört ins Menü-Paket, hier nicht verfolgt.
