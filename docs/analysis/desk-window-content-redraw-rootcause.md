# Root-Cause: Desktop-Fensterinhalt wird nach OPEN/NAVIGATE nicht (zuverlässig) gezeichnet

Status: READ-ONLY Root-Cause-Analyse (Auditor). Kein Produktcode geändert.
Build: HEAD `bb0b6b9`. Scope: die Content-Redraw-Kette der Directory-Fenster
(desk/ Trigger ↔ AES `wind_open`/Message-Pipe ↔ `win_bldview` ↔ `do_wredraw`).

> ## KORREKTUR (2026-07-22) — die eigentliche Wurzel war das fehlende `appl_init`
>
> Dieses Dokument benennt unten (R1) das **AES-Single-Slot-Message-Modell (#125)**
> als PRIMÄRE Ursache. Das war **notwendig, aber nicht hinreichend**. Die
> Per-App-FIFO-Queue (#125) ist inzwischen gelandet (HEAD `8d77372`), und das leere
> Fenster **blieb trotzdem**. Der tatsächliche Root-Cause (headless-verifiziert):
>
> Der NewTOS-Desktop rief **nie `appl_init`** — er hatte damit **keinen
> AES-Prozess-Deskriptor** für seine PID (=1). `send_redraw` → `pd::post_msg(1,
> WM_REDRAW)` läuft in `pd::by_pid(1)` = `None` und wird **verworfen** — egal ob
> der Slot ein Ein-Slot oder eine FIFO-Queue ist, denn es existiert überhaupt
> keine Queue für PID 1. Deshalb gingen ALLE gequeuten Redraws aus MOVE/SIZE/TOP/
> CLOSE/OPEN verloren; Inhalt erschien nur auf den **Direkt-Zeichen-Pfaden** (Boot
> `redraw_desktop`, View-Switch `win_shwall`). Genau das erklärt „erster Draw
> zeigt Inhalt, MOVE wird deterministisch leer".
>
> EmuTOS: `references/emutos/desk/deskmain.c:1953` `gl_apid = appl_init();` — früh
> in `deskmain()`, vor Fenster-/Loop-Init; `appl_exit()` bei Shutdown (`:2166`).
> Der Desktop ist eine normale AES-App. Fix: `desk::run()` ruft `appl_init` früh
> (öffnet den Deskriptor + FIFO für PID 1), `appl_exit` auf dem Exit-Pfad. Nur
> `desk/`, keine Änderung am AES-Message-Pfad. Die #125-Queue bleibt korrekt und
> nötig (sie sammelt jetzt die rect-koaleszierten Redraws, statt sie zu verlieren).
> Die R1-Analyse unten ist damit **richtig im Detail, aber falsch in der Priorität**:
> ohne Deskriptor kommt der Post gar nicht erst bis zur Queue-Logik.

## Symptom (real Pi4)

- Laufwerksfenster (z. B. `C:\`) öffnet, Arbeitsbereich in ICON-Ansicht **leer =
  reines WEISS**; INFO-/Statuszeile (Dateizahl + Bytes) **korrekt**.
- Umschalten auf TEXT-Ansicht → Liste **erscheint** (laut Koordinator „kommt,
  aber NICHT IMMER" → intermittierend).
- In ein Unterverzeichnis navigieren → wieder **leer/weiß**.
- Boot-`C:\`-Fenster rendert seinen vollen Icon-Baum korrekt.

## Verdikt in einem Satz

Der **primäre Fehler liegt in der AES-Schicht**: das Prozess-Message-Modell hat
nur **einen einzigen Message-Slot** pro Applikation (`Aespd::msg`), kein FIFO.
Der Desktop ist EINE AES-App, die VIELE Fenster besitzt; jede Fensteroperation
postet in denselben Slot, und ein noch nicht abgeholtes Content-`WM_REDRAW` wird
vom nächsten Post **stillschweigend überschrieben (verloren)**. Genau das ist die
Signatur des intermittierenden Weiß. Das ist die #125-Familie desselben Defekts
wie P5/#123 — nur für OPEN statt für close/top. **Es ist KEIN Icon-Build-Bug und
KEine reine Module-1-desk/-Regression**, aber Module-1 hat den Defekt für den
OPEN-Pfad *freigelegt*, weil OPEN als einziger inhaltsführender Pfad **keinen
direkten Zeichenaufruf** mehr hat und sich allein auf die verlustbehaftete
Message verlässt.

Warum genau „weiß + korrekte Infozeile": Der AES-Frame-Composite malt den
Arbeitsbereich **selbst weiß** und zeichnet die Infozeile **unabhängig** aus
`w.info` — der eigentliche Inhalt (die Item-Objekte) kommt **nur** aus
`do_wredraw`, das das (verlorene) `WM_REDRAW` auslösen müsste.
Beleg: `tos-api/src/aes/window.rs:833-838` (Work area — `fill_rect(work, WHITE)`)
und `:824-828` (Info line — `draw_label(..., &w.info, ...)`).

---

## Beweiskette (quellen-belegt, CONFIRMED)

### 1. Der Arbeitsbereich wird vom Frame weiß gefüllt, die Infozeile separat gemalt

`draw_frame` (`tos-api/src/aes/window.rs:771`) malt:
- Infozeile aus `w.info`: `:824-828`.
- Arbeitsbereich **weiß**: `:833-838`
  `let work = calc(WC_WORK, kind, b); backend.fill_rect(work.x, work.y, work.w, work.h, WHITE, clip);`

`draw_frame` läuft bei jedem Frame-Composite: `wind_open` → `redraw_clip` →
`compose` → `draw_frame` (`window.rs:1088`, `:407-431`), und ebenso bei
`redraw_desktop` → `window::redraw()`.

Konsequenz: Nach `wind_open` ist der Arbeitsbereich **weiß** und die Infozeile
**korrekt** — **auch dann, wenn nie Inhalt gezeichnet wird**. Das erklärt das
Symptom exakt, ohne dass `do_wredraw` je gelaufen sein muss.

### 2. Der Inhalt kommt ausschließlich aus `do_wredraw` (Item-Objekte)

`do_wredraw` (`kernel/src/desk/redraw.rs:117-126`) zeichnet den Item-Baum
(`objc_draw_tree` über die Owner-Rechteckliste). Es wird ausgelöst durch:
- den `WM_REDRAW`-Handler (`kernel/src/desk/main.rs:97-107`), oder
- direkte Aufrufe (`win_shwall`, `navigate_window`, `redraw_desktop`).

Fehlt der `do_wredraw`-Aufruf, bleibt der weiße Frame-Arbeitsbereich stehen.

### 3. AES-Message-Pipe ist EIN Slot, kein FIFO — jeder Post überschreibt

`Aespd` (`tos-api/src/aes/pd.rs:26-35`): genau **ein** Slot
`msg: [i16; MSG_WORDS]` + `msg_pending: bool` (`:32-34`). Kommentar dort:
„a single pending 16-byte message".

`send_redraw` (`tos-api/src/aes/window.rs:582-602`) schreibt **bedingungslos**:
```
p.msg = msg;            // :598
p.msg_pending = true;   // :599
```
Kein Queue-Check, kein Append, keine Rechteck-Vereinigung. Jeder Post
**überschreibt** einen noch offenen Post desselben (oder eines anderen) Fensters.

`evnt_multi` (`tos-api/src/aes/event.rs:331-349`) holt genau **einen** Slot ab
(`if src.msg_pending { … src.msg_pending = false; fired |= MU_MESAG }`). Es gibt
keine Warteschlange, aus der ein zweiter Post nachrücken könnte.

Der Defekt ist im Code bereits dokumentiert: `broadcast_redraw`
(`window.rs:650-654`) — „NewTOS's process message slot holds a single pending
message … only the last-walked one's `WM_REDRAW` survives". Und `wind_close`
(`window.rs:1109-1136`, P5/#123) sowie `wind_set(WF_TOP)` (`:1329-1347`) und
`wind_set(WF_CXYWH)` (`:1310-1328`) posten alle über dasselbe `send_redraw` in
denselben Slot.

### 4. Der OPEN-Pfad verlässt sich AUSSCHLIESSLICH auf diese eine Message

`open_dir_window` (`kernel/src/desk/main.rs:1556-1616`):
- `wind_open(handle, r)` (`:1602`) → AES `send_redraw` → Content-`WM_REDRAW` in
  den Ein-Slot (`window.rs:1089`).
- danach `win_bldview` (`:1607`), `set_sliders`, `desk_verify(…, false)`.
- **kein** direkter Content-Zeichenaufruf. Der Kommentar `:1611-1615` sagt es
  ausdrücklich: „No desktop-side repaint: `wind_open` … `send_redraw`d a
  `WM_REDRAW` … drained by the event loop into `redraw::do_wredraw`".

Wird dieses eine `WM_REDRAW` überschrieben, bevor die Event-Schleife es abholt,
gibt es **keinen Ersatz** → weißes Fenster.

### 5. Warum Boot / View-Switch / Navigate-up funktionieren (das Differenzial)

Alle drei zeichnen den Inhalt **direkt**, unabhängig von der Message:
- **Boot**: `run()` ruft nach `open_root_window` explizit `redraw_desktop()`
  (`kernel/src/desk/mod.rs:550-552`) → `win_shwall_nocursor` → `do_wredraw` je
  Fenster (`redraw.rs:206-215`, `:153-164`). Direkt. → immer korrekt.
- **View-Switch**: `do_viewmenu` → `desk_all` → `win_shwall()`
  (`main.rs:1287-1295`) → direkter `do_wredraw`. → zuverlässig.
- **Navigate-up** (Close-Folder, `!new_win`): `navigate_window`
  (`main.rs:1639-1678`) ruft am Ende **direkt** `redraw::do_wredraw(handle, work)`
  (`:1677`). → korrekt (und deckungsgleich mit EmuTOS, s. u.).

Nur **OPEN eines neuen Fensters** (`open_dir_window`) — und damit auch das
„Navigieren IN ein Unterverzeichnis", denn das ist per `main.rs:339`
(`OpenTarget::Folder → open_dir_window`) ebenfalls ein NEUES Fenster — verlässt
sich allein auf die verlustbehaftete Message. Daher exakt dieser Pfad weiß.

Das erklärt auch das scheinbare „Icon leer, Text erscheint"-Muster: Icon war
die Ansicht beim OPEN (Message-Pfad, droppable → weiß); Text „erscheint", weil
der Nutzer danach die Ansicht wechselt und dabei `win_shwall` (direkt) läuft —
**nicht** weil Icons unzeichenbar wären. `win_bldview` baut in BEIDEN Ansichten
denselben Item-Baum (`kernel/src/desk/win.rs:285-450`); der Boot-Icon-Render
beweist, dass `G_ICON` über `do_wredraw` sauber zeichnet. **Der Icon-Build ist
nicht der Fehler.**

---

## Gegenprobe mit EmuTOS (Ground Truth)

EmuTOS macht strukturell dasselbe (Frame-Composite + `WM_REDRAW` an den Owner),
verliert die Message aber **nie**, weil jede App eine echte **FIFO-Queue** hat.

- AESPD hat eine Byte-Queue: `p_qaddr` / `p_qindex` / `p_queue[QUEUE_SIZE]`
  (`references/emutos/aes/struct.h:150-152`), `QUEUE_SIZE = 128`
  (`struct.h:56`) = bis zu 8 Messages à 16 Byte.
- Enqueue = echtes FIFO mit Sonderregel: `doq`
  (`references/emutos/aes/gemqueue.c:31-82`) hängt die 16 Byte an `p_qindex` an
  (`:39,:73`); bei `WM_REDRAW` sucht es eine bereits vorhandene `WM_REDRAW`
  **desselben Fensters** und **vereinigt die Rechtecke** statt zu überschreiben
  (`:45-56`, `rc_union`). Nichts geht verloren; gleiche Fenster koaleszieren.
- `wind_open` → `wm_open` (`references/emutos/aes/gemwmlib.c:1410-1432`) →
  `draw_change` (`:1058-1096`, `:1219-1222`) → `w_update`
  (`:1004-1048`) → `w_redraw` (`:887-907`) → `ap_sendmsg`
  (`:851-863`) → `ap_rdwr(MU_SDMSG, …)` → `doq` (Queue). Also: **neues Fenster
  → gequeuetes `WM_REDRAW`**, zuverlässig zugestellt.
- `do_diropen` (`references/emutos/desk/desksupp.c:422-471`): baut/verifiziert
  die View (`desk_verify(pw->w_id, TRUE)`, `:462`) und sendet ein **eigenes**
  `WM_REDRAW` **nur für `!new_win`** (`:465-466`). Für ein NEUES Fenster kommt
  der Redraw aus dem AES `wm_open` (oben). Für Navigate im selben Fenster geht
  EmuTOS über `fun_msg`, und `fun_msg` (`references/emutos/desk/deskfun.c:101-112`)
  ruft `hndl_msg()` **synchron sofort** → `do_wredraw` direkt
  (`references/emutos/desk/deskmain.c:1275-1281`).

Gegenüberstellung:

| Pfad | EmuTOS | NewTOS | Ergebnis NewTOS |
|---|---|---|---|
| Neues Fenster öffnen | AES `wm_open` → **queued** `WM_REDRAW` | AES `wind_open` → **Ein-Slot**-`send_redraw` | droppable → weiß (intermittierend) |
| Navigate im Fenster | `fun_msg` → `hndl_msg` **synchron** `do_wredraw` | `navigate_window` **direkt** `do_wredraw` | korrekt |
| View/Sort | `win_bdall`+`win_shwall` direkt | `desk_all`→`win_shwall` direkt | korrekt |

NewTOS' desk/-Struktur ist also **EmuTOS-treu**; die Divergenz ist die
**Ein-Slot-Message-Pipe** statt der FIFO-Queue.

---

## Rangfolge der Ursachen

### R1 — AES Single-Slot-Message-Modell (#125). PRIMÄR. CONFIRMED (quellenbelegt).

- Layer: **AES** (`tos-api/src/aes/pd.rs:32-34`, `window.rs:582-602`,
  `event.rs:331-349`).
- Divergenz: Ein-Slot-Überschreiben statt FIFO-Queue mit Rechteck-Union
  (EmuTOS `struct.h:150-152`, `gemqueue.c:31-82`).
- Fehlerszenario: Der Desktop (eine App, viele Fenster) postet aus
  `wind_open`/`wind_set(WF_TOP)`/`wind_set(WF_CXYWH)`/`wind_close`-Broadcast in
  denselben Slot. Landen zwei Posts, bevor `evnt_multi` abholt, geht der erste
  (oft das Content-`WM_REDRAW`) verloren → `draw_frame`-Weiß bleibt stehen,
  Infozeile korrekt.
- Intermittenz: hängt von Gesten-/Maus-Timing ab (ob ein zweiter Post
  hineinrennt) → „mal weiß, mal nicht". Signatur eines gedroppten Messages.
- Bug vs. akzeptable Abweichung: **echter Bug**, durch die dokumentierte API
  beobachtbar (Fensterinhalt fehlt). Betrifft **nicht nur den Desktop**: JEDE
  echte GEM-App (QED etc.) verliert Redraws, wenn zwei Messages gleichzeitig
  anstehen (P5/#123 war exakt das für close/top).

### R2 — OPEN-Pfad ohne direkten Content-Zeichenaufruf. BEITRAGEND. CONFIRMED.

- Layer: **desk/** (`kernel/src/desk/main.rs:1556-1616`, insb. `:1611-1615`).
- Divergenz: NewTOS' `navigate_window` (`:1677`), Boot (`mod.rs:552`) und
  View-Switch (`main.rs:1294`) zeichnen Inhalt **direkt**; `open_dir_window`
  **nicht** — es verlässt sich allein auf R1. Damit ist OPEN der einzige
  inhaltsführende Pfad mit **null** Toleranz gegen den R1-Drop.
- Verhältnis zu EmuTOS: Für ein NEUES Fenster verlässt sich auch EmuTOS
  `do_diropen` auf die AES-Message (nicht auf einen direkten `do_wredraw`),
  `desksupp.c:465-466`. `open_dir_window` ist also **treu**; erst R1 macht es
  fatal. **Es ist damit keine echte desk/-Regression, sondern eine durch
  Module-1 freigelegte Anfälligkeit gegen den AES-Defekt.**
- Bug vs. akzeptable Abweichung: für sich genommen keine Abweichung; nur in
  Kombination mit R1 beobachtbar.

### Ausgeschlossen (mit Beleg)

- **`win_bldview`/Icon-Pfad**: KEIN Fehler. Boot rendert den Icon-Baum über
  denselben `win_bldview` + `do_wredraw` korrekt; beide Ansichten bauen denselben
  Baum (`win.rs:401-450`). Das „Text ja, Icon nein" ist ein Trigger-, kein
  Zeichenunterschied.
- **`do_wredraw`-Clip**: KEIN Fehler. Der Clip aus `wind_open`
  (`calc(WC_WORK, kind, r)`, `window.rs:1089`) entspricht dem Work-Rect; identisch
  zum direkten Pfad. `compute_rlist` liefert für das oberste, gerade geöffnete
  Fenster die volle Work-Area (`window.rs:537-563`).

---

## Fix-Richtung (Reihenfolge)

1. **PRIMÄR / Wurzel-Fix (#125): echte Per-App-Message-QUEUE im AES.**
   `Aespd` auf ein FIFO umstellen, das EmuTOS `gemqueue` spiegelt: `p_queue`
   (Bytes), `ap_rdwr`-artiges enqueue/dequeue, und die
   `WM_REDRAW`-Gleiches-Fenster-Rechteck-Union (`gemqueue.c:45-56`).
   `send_redraw`/`broadcast_redraw`/`send_redraw_in` enqueuen statt zu
   überschreiben; `evnt_multi`/`evnt_mesag` dequeuen den Kopf. Das behebt OPEN
   **und** den `wind_close`-Broadcast (P5-Fan-out), `WF_TOP`, `WF_CXYWH` **und**
   alle echten GEM-Apps in einem Zug. → an Developer.

2. **SEKUNDÄR / Sofort-Mitigation (defense-in-depth, desk-seitig): direkter
   Content-Redraw in `open_dir_window`.** Nach `win_bldview`/`desk_verify` einen
   `redraw::do_wredraw(handle, work)` anhängen (wie `navigate_window` es bereits
   tut, `main.rs:1677`). Macht den Desktop-OPEN-Pfad unabhängig von der
   Message-Zustellung und spiegelt das, was Boot mit `redraw_desktop` bereits
   tut. Behebt **nur** den Desktop, nicht echte Apps — daher zusätzlich zu (1),
   nicht statt (1).

Empfehlung: **beide**, die Queue zuerst (Wurzel). Die desk-seitige Direktzeichnung
ist ein sicheres Interim entlang des bereits bewiesenen Boot-Ansatzes.

---

## Was getrieben wurde vs. offen

**Getrieben / quellen-bewiesen (CONFIRMED):**
- Vollständige Call-Path-Analyse aller vier inhaltsführenden Pfade
  (OPEN/NAVIGATE-in, NAVIGATE-up, VIEW-SWITCH, BOOT) und ihrer Trigger.
- End-to-End-Beweis der Symptom-Mechanik: `draw_frame`-Weiß + separate Infozeile
  (`window.rs:824-838`) + Content nur via `do_wredraw` + Ein-Slot-Überschreiben
  (`send_redraw` `window.rs:598-599`, `pd.rs:32-34`, `event.rs:331-349`).
- EmuTOS-Gegenprobe: FIFO-Queue mit Rechteck-Union (`gemqueue.c:31-82`,
  `struct.h:150-152`), `wm_open`→queued `WM_REDRAW`
  (`gemwmlib.c:1410-1432`,`:887-907`,`:851-863`), `do_diropen`/`fun_msg`
  (`desksupp.c:465-466`, `deskfun.c:101-112`).

**Nicht headless reproduzierbar (PLAUSIBLE, braucht Real-HW/Interaktion):**
- Die **exakte** konkurrierende Zweit-Message pro Geste, die den OPEN-Redraw im
  jeweiligen Fall droppt, und damit das genaue Intermittenz-Muster. Interaktives
  Öffnen/Navigieren braucht Eingabe UND ein C:-Laufwerk; die QEMU-Boards trennen
  das (raspi4b hat C:, aber Input aus; qemu-virt hat Input, 0 Laufwerke). Der
  Ein-Slot-Drop selbst ist jedoch quellen-bewiesen (nicht nur reasoned).
- Die vom Nutzer berichtete Text-Ansicht-Intermittenz: `win_shwall` zeichnet
  direkt und sollte zuverlässig sein; die „nicht immer"-Beobachtung deckt sich
  am ehesten mit dem OPEN-Weiß (Icon war die Ausgangsansicht). Ob es eine
  eigenständige Text-Flakiness gibt, **braucht das Real-HW-Foto zur Bestätigung**.

**Empfohlener Treiber-Test (nicht ausgeführt — Begründung unten):**
Ein Selftest analog zu `kernel/src/selftest_objc.rs:699 run_p5_redraw` reproduziert
den Drop deterministisch über den echten GEM-Trap: `appl_init`; `wind_create`+
`wind_open A` → `test_pending_msg(ap)` zeigt `WM_REDRAW` für A; **ohne** Drain
`wind_open B` → `test_pending_msg(ap)` zeigt jetzt nur B, A ist weg. Das beweist
„zweiter Post droppt den ersten". Nicht ausgeführt, weil (a) der Mechanismus
bereits durch drei eindeutige Quellzeilen (`send_redraw` überschreibt
bedingungslos) CONFIRMED ist und (b) das `Edit`-Tool in dieser Sitzung nicht
verfügbar war; ein `Write` auf die 1100-Zeilen-Datei hätte fremde, nicht
committete Arbeit anderer Tasks gefährdet. Der Recipe steht dem Developer bereit.

## Out-of-scope-Beobachtungen (nicht verfolgt)

- `wind_close` (`window.rs:1109-1136`) + `broadcast_redraw` (`:655-662`) verlieren
  bei mehreren gleichzeitig freigelegten Fenstern alle bis auf ein `WM_REDRAW` —
  derselbe R1-Defekt (dokumentiert als P5/#123); die Queue behebt auch das.
- `close_window` (`main.rs:1620-1635`) re-layoutet untenliegende Fenster nicht
  (verlässt sich auf den AES-Broadcast, der droppt) — ebenfalls R1.

## Cleanup

Kein Produktcode geändert. Der frühere `Edit`-Versuch an `selftest_objc.rs`
schlug fehl (Tool-Fehler), es wurde nichts geschrieben; `git status` bestätigt,
dass `selftest_objc.rs` unverändert ist. Die übrigen im Baum als geändert
angezeigten Dateien (`kernel/src/lib.rs`, `exceptions.rs`, `.claude/agents/*`
etc.) stammen aus anderer, nicht committeter Arbeit und wurden von diesem Audit
**nicht** angefasst. Einziges neu erstelltes Artefakt: dieses Dokument.
