# Die 64 Abweichungen der AES-Unterfunktionstabelle — Einordnung

**Datum:** 2026-09-12 · **Grundlage:** `measurement-2026-09-12-aes-r4/`,
Falltabelle `tosdump/cases/aes-r4-subfunctions.tsv` (181 Fälle, CRC `227422B2`) ·
**Quelle der Wahrheit:** `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(bestätigt: `git -C references/emutos log -1` → *Update GitHub Actions to use Node 24*,
2026-04-14).

Auftrag 1 hat gemessen und ausdrücklich nicht gedeutet. Dies ist die Deutung.
Jeder der 64 Fälle steht in **genau einer** Schublade, mit Beleg auf beiden
Seiten.

---

## 0. Nachprüfung der Auswahl

Die 64 Fälle stammen aus fremder Hand und wurden vor der Deutung neu aus
`DIFF-emutos-vs-newtos-640x480.txt` extrahiert (eigener Parser, eigener
BIN-Dekoder gegen `tosdump/core/tosdump.h:114-124`).

| geprüft | Auftrag 1 | nachgerechnet |
|---|---|---|
| Fälle mit Unterschied ausserhalb `global[]` | 64 | **64** |
| Verteilung nach Opcode | 0(1) 10(1) 26(2) 40(3) 46(7) 77(1) 79(1) 104(20) 112(28) | **identisch** |

Auch der deklarierte Versionsstand wurde nachgeprüft:
`git log VERSION_1_4..1a7484ab -- aes/ include/aesdefs.h include/obdefs.h include/gemdefs.h`
liefert **genau zwei** Commits — `acb7f3b4` (WF_OWNER) und `4b0d9518` (nicelines).
Eine dritte Versionsdifferenz behaupte ich nicht; es gibt keine.

---

## 1. Bilanz

| Schublade | Fälle |
|---|---|
| **1 — NewTOS ist falsch** | **36** |
| **2 — Versionsdelta** | **2** |
| **3 — Geräteparameter oder kein Soll** | **26** |
| Summe | 64 |

Dahinter stehen **sechs** unterscheidbare Defekte, nicht 36 — die 36 Fälle sind
sechs Ursachen, die mehrfach getroffen werden.

---

## 2. Schublade 1 — NewTOS ist falsch

### D-1 · `graf_mkstate` (79): Mauszeiger startet einen Pixel zu weit rechts/unten

| | |
|---|---|
| **Fall** | 286 (`79 0 5 0 0 -`) |
| **Was der Fall prüft** | die vier Wörter, die `gr_mkstate` schreibt |
| **EmuTOS** | `int_out[1..4] = 319, 239, 0, 0` |
| **NewTOS** | `int_out[1..4] = 320, 240, 0, 0` |
| **Verdikt** | **CONFIRMED** — zweimal unabhängig gemessen (r4-Dump und eigener Probelauf, Abschnitt 6) |

EmuTOS setzt den Zeiger auf `GCURX = xres/2`, `GCURY = yres/2`
(`vdi/vdi_mouse.c:582-583`), und `xres`/`yres` sind **`DEV_TAB[0]`/`DEV_TAB[1]`**
(`vdi/vdi_defs.h:94`), also `V_REZ_HZ - 1` / `V_REZ_VT - 1`
(`vdi/vdi_control.c:203-204`). Bei 640×480 ergibt das `639/2 = 319` und
`479/2 = 239`.

NewTOS rechnet mit der **vollen** Breite:

```rust
// tos-api/src/vdi/mouse.rs:321-322
GCURX.store(backend.width() as i16 / 2, Ordering::Relaxed);
GCURY.store(backend.height() as i16 / 2, Ordering::Relaxed);
```

Das ist eine örtliche Verwechslung, kein systematischer `xres`-Fehler: an den
übrigen Stellen steht `backend.width() as i16 - 1` (`vdi/attributes.rs:50`,
`vdi/gdp.rs:283`, und im selben Modul `vdi/mouse.rs:202` für die Klemmung).

**Fehlerbild über die dokumentierte API:** `graf_mkstate`, `vq_mouse`,
`evnt_multi(MU_BUTTON)` melden nach dem Start `320/240` statt `319/239`; der
Zeiger wird einen Pixel versetzt gezeichnet. Bei 1920×1080 entsprechend
`960/540` statt `959/539`. Kleine Schwere, belegter Unterschied, billige
Reparatur.

**Ticket:** keines vorhanden → **neu**.

---

### D-2 · `wind_get(WF_SCREEN)` (104/17): vier Nullwörter statt Puffer

| | |
|---|---|
| **Fälle** | 331 (`0,17`), 332 (`-1,17`), 439 (`1,17`) — 3 |
| **Was die Fälle prüfen** | die zwei LONGs, die `gsx_mret` schreibt; 332 zusätzlich, dass kein gültiges Handle nötig ist |
| **EmuTOS** | `int_out[1..4] = 0x0100, 0x0218, 0x0000, 0xA000` → Adresse `0x01000218`, Länge `40960`; Rückgabe 1 |
| **NewTOS** | `int_out[1..4] = 0, 0, 0, 0`; Rückgabe 1 |
| **Verdikt** | **CONFIRMED** |

EmuTOS: `case WF_SCREEN: gsx_mret((LONG *)poutwds, (LONG *)(poutwds+2));`
(`aes/gemwmlib.c:1563-1565`), und `gsx_mret` gibt `gl_tmp.fd_addr` und `gl_mlen`
zurück (`aes/gemgsxif.c:125-129`). Der Puffer wird **einmal beim AES-Start**
belegt: `gl_tmp.fd_addr = dos_alloc_anyram(mlen)` (`:113`) mit
`gl_mlen = MENU_BUFFER_SIZE * gl_wchar * gl_hchar * gl_nplanes / 8` (`:101`).
Die gemessenen `40960 = 640 · 8 · 16 · 4 / 8` bestätigen die Formel.

NewTOS gibt hartverdrahtete Nullen zurück:

```rust
// tos-api/src/aes/window.rs:2672-2679
if field == WF_SCREEN {
    for i in 1..=4 {
        pb.set_intout(i, 0); // address (hi/lo) + length (hi/lo)
    }
    pb.set_ret(1);
    return;
}
```

Der Kommentar darüber führt das als „Assessed deliberate deviation
(AUD-P5-012)". **Dem widerspreche ich.** Eine Begründung ist keine
Unbeobachtbarkeit. `wind_get(WF_SCREEN)` ist dokumentierte API; ein Programm,
das sich den AES-Sicherungspuffer als Kratzfläche geben lässt — genau wofür er
da ist —, bekommt auf NewTOS die Adresse `0` und die Länge `0` und schreibt
gegen die Null. Der Rückgabewert `1` behauptet dabei Erfolg. Das ist zugleich
ein Grenzfall der Regel 2: ein fester Wert, der die Argumente ignoriert.

Die Abhilfe ist keine Fassade, sondern ein wirklicher persistenter Puffer — den
die AES für Menü- und Alarm-Sicherung ohnehin braucht.

**Ticket:** im Inventar als **A-6** geführt, dort mit Status *„bewusste
Abweichung, gelesen"*; jetzt **gemessen**. Kein Ticket in
`TASK/GAPS-INDEX-2026-09-12.md` → **neu**, und der Status im Inventar gehört
korrigiert (gehört zu [A53](../../TASK/A53.md)).

---

### D-3 · `objc_add` (40): Rückgabe 0 statt 1 bei `NIL`-Argument

| | |
|---|---|
| **Fälle** | 550 (`-1,-1`), 551 (`-1,0`), 552 (`0,-1`) — 3 |
| **Was die Fälle prüfen** | den Rückgabewert auf dem `NIL`-Zweig, den `ob_add` vor der ersten Baumberührung nimmt |
| **EmuTOS** | `int_out[0] = 1` in allen drei Fällen |
| **NewTOS** | `int_out[0] = 0` in allen drei Fällen |
| **Verdikt** | **CONFIRMED** |

EmuTOS' Dispatcher setzt `ret = TRUE` vor dem `switch`
(`aes/gemsuper.c:84-85`), und der `OBJC_ADD`-Zweig fasst `ret` **nicht** an:

```c
/* aes/gemsuper.c:204-206 */
case OBJC_ADD:
    ob_add((OBJECT *)OB_TREE, OB_PARENT, OB_CHILD);
    break;
```

`ob_add` ist `void` und hat nur einen inneren Schutz
(`if ((parent != NIL) && (child != NIL))`, `aes/gemoblib.c:1121-1141`).
`objc_add` liefert damit **immer** `1`.

NewTOS erfindet einen Fehlerrückgabewert:

```rust
// tos-api/src/aes/object.rs:1071-1081
pub fn objc_add(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let parent = pb.intin(0);
    let child = pb.intin(1);
    if tree.is_null() || parent < 0 || child < 0 {
        pb.set_ret(0);
        return;
    }
    ...
```

**Fehlerbild:** ein Programm, das seinen Baum zur Laufzeit zusammensetzt und
`if (!objc_add(tree, parent, child)) fehler();` schreibt, meldet auf NewTOS
einen Fehler, den es auf TOS und EmuTOS nie gibt. Die *Wirkung* auf den Baum ist
in beiden Fällen dieselbe (nichts) — nur der Rückgabewert lügt.

**Ticket:** keines vorhanden, auch nicht im Inventar (dort ist unter A-8 nur
`objc_order` erfasst) → **neu**.

---

### D-4 · `rsrc_gaddr` (112): Rückgabe 0 für jeden definierten `R_*`-Typ

| | |
|---|---|
| **Fälle** | 598–614 (Typ 0..16, Index 0), 616–621 (die drei Identitätspaare bei Index 1), 626 (`R_OBJECT` Index 1000), 627 (`R_OBJECT`, `n_addrout 0`) — **25** |
| **Was die Fälle prüfen** | den **Rückgabecode je Typ** (die eine Frage, die ohne geladene Resource entscheidbar ist) |
| **EmuTOS** | `int_out[0] = 1` in allen 25 Fällen |
| **NewTOS** | `int_out[0] = 0` in allen 25 Fällen |
| **Verdikt** | **CONFIRMED** |

`rs_gaddr` liefert `(*rsaddr != (void *)-1L)` (`aes/gemrslib.c:717-722`), und
`get_addr` gibt `-1L` **nur** im `default:`-Zweig zurück
(`aes/gemrslib.c:170-172`); jeder definierte Typ rechnet eine Adresse aus, auch
aus einem `NULL`-Header. Also antwortet EmuTOS für jeden der 17 Typen `TRUE`.

NewTOS prüft dagegen Tabellenvorhandensein **und** Index:

```rust
// tos-api/src/aes/resource.rs:1430-1442
fn array_slot(target_base: u64, access_base: u64, count: usize,
              size: usize, off: u64, index: usize) -> Option<Slot> {
    if target_base == 0 || index >= count {
        return None;
    }
    ...
```
und `rsrc_gaddr` macht daraus `addr_out = 0`, `ret = 0`
(`tos-api/src/aes/resource.rs:1582-1591`).

Zwei Dinge sind dabei **nicht** falsch und sollen nicht mitgezählt werden:

* Der **absolute `addr_out[0]`** hat kein Soll — das steht so im Tabellenkopf
  und ist richtig: `rs_hdr` ist auf beiden Maschinen `NULL`
  (`aes/gemrslib.c:677-678`), EmuTOS rechnet aus der m68k-Vektortabelle.
* Fall **627** beantwortet eine eigene Frage — schreibt der Dispatcher
  `addrout[0]` auch bei `n_addrout = 0`? EmuTOS tut es bedingungslos
  (`if (OP_CODE == RSRC_GADDR) pcrys_blk->addrout[0] = (LONG)ad_rso;`,
  `aes/gemsuper.c:446-447`). NewTOS **auch** — der Wert kam als `0` zurück, nicht
  als `0xDEADDEAD`-Gift. **Diese Teilfrage geht gleich aus.**

**Die drei Identitätspaare** (2≡8, 3≡11, 4≡14, je Index 0 und 1) halten auf der
EmuTOS-Seite alle sechs: `0xE0/0xE0`, `0xFC/0xFC`, `0x30/0x30`, `0x52/0x52`,
`0xE0/0xE0`, `0xEE/0xEE`. Die Schrittweiten bestätigen dabei die
m68k-Satzgrössen: TEDINFO 28, ICONBLK 34, BITBLK 14. Auf der NewTOS-Seite ist
die Identitätsfrage **gegenstandslos**: alle sechs Werte sind `0`, weil der
Schutz vor der Rechnung greift. Die Gleichheit gilt trivial und sagt nichts.
Das muss so dastehen und darf nicht als „bestanden" gebucht werden.

**Fehlerbild über die dokumentierte API:** die gemessene Vorbedingung („keine
Resource geladen") erreicht ein konformes Programm nicht. Erreichbar ist die
Form von Fall **626**: mit geladener Resource nach einem Index **jenseits** der
Tabelle fragen — EmuTOS antwortet `TRUE` samt Zeiger ins Nichts, NewTOS `FALSE`.
Ebenso: nach `R_BITBLK` in einer Resource **ohne** BITBLK-Abschnitt fragen.

**Ticket:** bestätigt **[A50](../../TASK/A50.md)** (Inventar-Befund A-4)
vollständig — die Messung macht aus „gelesen" ein „gemessen". A50s Haltung
(entscheiden, nicht blind angleichen) bleibt richtig: Angleichen hiesse hier,
eine Indexprüfung zu **entfernen**.

---

### D-5 · `rsrc_gaddr` (112) beim **unbekannten** Typ: `addr_out[0] = 0` statt `−1`

| | |
|---|---|
| **Fälle** | 623 (Typ 17), 624 (Typ 99), 625 (Typ −1 ⇒ 65535) — 3 |
| **Was die Fälle prüfen** | die Fallback-Klasse: unbekannter `R_*`-Typ |
| **EmuTOS** | `int_out[0] = 0`, `addr_out[0] = 0xFFFFFFFF` |
| **NewTOS** | `int_out[0] = 0`, `addr_out[0] = 0x00000000` |
| **Verdikt** | **CONFIRMED** |

Hier stimmt der Rückgabecode auf beiden Seiten. Der **Adresswert** ist es, der
abweicht — und anders als bei D-4 hat er hier sehr wohl ein Soll: der
`default:`-Zweig von `get_addr` fasst `rs_hdr` **nie** an und liefert die
Konstante `-1L` (`aes/gemrslib.c:170-172`); `rs_gaddr` schreibt sie über
`*rsaddr = get_addr(...)` in den Ausgabezeiger (`:717-721`), und
`gemsuper.c:446-447` reicht sie an `addrout[0]` durch. `-1` ist damit
geräteunabhängig und exakt vorhergesagt — der Vorbehalt des Tabellenkopfes
(„der absolute `addr_out[0]` hat kein Soll") gilt für diese drei Fälle **nicht**.

NewTOS: `unsafe { get_addr(rtype, index) }.map_or(0, |s| s.target)`
(`tos-api/src/aes/resource.rs:1586`) — `None` wird zu `0`, nicht zu `-1`.

**Fehlerbild:** ein Programm, das `rsrc_gaddr` mit einem Typ aus einer neueren
Bindung aufruft und den Rückgabewert ignoriert (verbreitet), bekommt auf TOS
`-1` — den klassischen „ungültig"-Zeiger, den viele Bindungen selbst prüfen —
und auf NewTOS `NULL`. Kleine Schwere, aber ein reiner Wertfehler ohne
Robustheits-Abwägung: die Angleichung kostet **nichts** und entfernt keine
Prüfung. Das trennt D-5 von D-4/A50.

**Ticket:** von A50 **nicht** erfasst (A50 spricht von der Indexprüfung) →
**neu**, oder als zweiter Punkt an A50 anhängen.

---

### D-6 · `appl_init` (10): `ap_id` 1 statt 0

| | |
|---|---|
| **Fall** | 644 (`10 0 2 0 0 -`, Wiedereintritt) |
| **Was der Fall prüft** | was `ap_init()` auf einem bereits offenen Prozess zurückgibt |
| **EmuTOS** | `int_out[0] = 0` |
| **NewTOS** | `int_out[0] = 1` |
| **Verdikt** | **CONFIRMED** als Abweichung; das Fehlerbild ist **PLAUSIBLE** |

`ap_init()` gibt `rlr->p_pid` zurück (`aes/gemaplib.c:54-63`), und
`gemsuper.c:94` legt denselben Wert in `ap_id` ab. In EmuTOS ist **pid 0** der
anfängliche Prozess, der zum Desktop wird (`aes/geminit.c:875-876`), und
**pid 1 ist fest der Screen Manager** (`aes/geminit.c:884-888`, Kommentar
*„it has a pid == 1"*). Ein vom Desktop gestartetes Programm läuft in dessen
AESPD und bekommt daher `0` — was der TT-Lauf bestätigt.

NewTOS vergibt Slot + 1:

```rust
// tos-api/src/aes/appl.rs:191-199
pub(super) fn current_pid() -> i16 {
    let bp = crate::bdos::process::current();
    if bp.is_null() { return 1; }
    crate::bdos::process::slot_for(bp).map(|s| s as i16 + 1).unwrap_or(1)
}
```

Der Unterschied ist im `global[]`-Rauschen als bekannt geführt — aber „bekannt"
ist ein Etikett, kein Beweis. Fall 644 zeigt ihn **ausserhalb** von `global[]`,
im Rückgabewert von `appl_init` selbst, und genau deshalb gehört er hier
eingeordnet.

**Fehlerbild (nicht gefahren, daher PLAUSIBLE):** Nachrichten tragen die
Absender-pid (`ap_msg[1] = rlr->p_pid`, `aes/gemwmlib.c:855`); auf EmuTOS
kommen die `WM_*`-Nachrichten des Control Managers mit `msg[1] == 1` und sind
damit von der eigenen pid `0` unterscheidbar. Auf NewTOS **ist** die eigene pid
1. Wer `msg[1] == gl_apid` prüft, trifft die falsche Entscheidung.

Das zweite Wort des Falls (`int_out[1]`: EmuTOS `7`, NewTOS `0`) gehört nicht
hierher — `appl_init` schreibt es nie; es ist Stapelrest (siehe Schublade 3).
Der Fall zählt als **eine** Abweichung, eingeordnet nach dem Wort mit Soll.

**Ticket:** keines vorhanden → **neu**. (Der R3-Bericht hatte nur die
`ap_private`-Wortlage für ein eigenes Ticket vorgemerkt, nicht `ap_id`.)

---

## 3. Schublade 2 — Versionsdelta

### `wind_get(WF_OWNER)` (104/20) — Commit `acb7f3b4`

| | |
|---|---|
| **Fälle** | 334 (`0,20` auf dem Desktop-Handle), 442 (`1,20` auf einem offenen Fenster) — 2 |
| **EmuTOS 1.4** | 334: `int_out = 0, 0, 0x0218, 0, 0xA000` · 442: `int_out = 0, 0x0100, 0x0218, 0, 0xA000` |
| **NewTOS** | 334: `int_out = 1, 0, 0, -1, -1` · 442: `int_out = 1, 1, 1, 1, 1` |
| **Verdikt** | **CONFIRMED** als Versionsdelta |

Die 1.4-ROM hat für Feld 20 keinen Zweig; `wm_get` fällt in
`default: return FALSE` (`aes/gemwmlib.c:1590-1592`) und schreibt **nichts** —
die vier Wörter sind exakt die Antwort des vorigen `WF_SCREEN`-Falls
(`0x0100, 0x0218, 0x0000, 0xA000`), also Stapelrest. Das ist die Signatur, die
das Versionsdelta unabhängig von der Deklaration belegt.

Die gepinnte Quelle hat den Zweig seit `acb7f3b4` (2025-08-23), gegattert mit
`#if AES_VERSION >= 0x330` — unser Profil meldet `0x0340`, also aktiv:

```c
/* aes/gemwmlib.c:1578-1589 */
case WF_OWNER:
    poutwds[0] = pwin->w_owner->p_pid;
    poutwds[1] = (pwin->w_flags & VF_ISOPEN) != 0;
    poutwds[2] = gl_wtop;
    poutwds[3] = W_TREE[ROOT].ob_head;
```

**Zusätzlich geprüft — NewTOS' Fassung stimmt mit der gepinnten Quelle
überein**, Wort für Wort (`tos-api/src/aes/window.rs:2720-2733`):

| Wort | gepinnte Quelle | Fall 334 (nichts offen) | Fall 442 (Fenster 1 offen) |
|---|---|---|---|
| `poutwds[0]` | `w_owner->p_pid` | `0` — Desktopfenster gehört pid 0 (`wm_init` → `w_setup(rlr, DESKWH, NONE)`, `aes/gemwmlib.c:1290`) | `1` — das Fenster des Dumpers |
| `poutwds[1]` | `VF_ISOPEN` | `0` — Handle 0 wird nie `wm_open`t | `1` |
| `poutwds[2]` | `gl_wtop` **roh**, nicht `w_top()` | `-1` (`NIL`) | `1` |
| `poutwds[3]` | `W_TREE[ROOT].ob_head` | `-1` (`NIL`) | `1` |

Auch die Rückgabe `1` für Handle 0 ist richtig: `WF_OWNER` steht nicht in der
Ausnahmeliste `WF_TOP`/`WF_SCREEN`/`WF_DCOLOR` (`aes/gemwmlib.c:1517-1524`),
braucht also ein gültiges Handle — und Handle 0 **ist** gültig
(`get_pwin`, `:1391-1404`, mit `VF_INUSE` aus `w_setup`).

**Kein Defekt, kein Ticket.** Die Messung bestätigt umgekehrt, dass NewTOS die
neuere Fassung korrekt trägt.

---

## 4. Schublade 3 — Geräteparameter oder kein Soll

### S3-a · Der nicht beschriebene Teil von `int_out[]` (23 Fälle)

EmuTOS' Dispatcher legt `WORD int_out[O_SIZE]` **uninitialisiert** auf den Stapel
und kopiert danach `OUT_LEN` Wörter zurück — ohne Rücksicht darauf, wie viele
die Funktion beschrieben hat:

```c
/* aes/gemsuper.c:429-432, 443-445 */
static void xif(AESPB *pcrys_blk)
{
    WORD    control[C_SIZE];
    WORD    int_in[I_SIZE];
    WORD    int_out[O_SIZE];
    ...
    if (OUT_LEN)
        memcpy(pcrys_blk->intout, int_out, OUT_LEN*sizeof(WORD));
```

Jedes nicht beschriebene Wort ist damit unbestimmter C-Speicher — in der Praxis
der Rest des vorigen Aufrufs. NewTOS nullt den gleichen Puffer:

```rust
// tos-api/src/aes/mod.rs:259-263
// `xif`'s `WORD int_out[O_SIZE]` (`gemsuper.c:432`). EmuTOS leaves it
// uninitialised — indeterminate C auto storage, i.e. no value to
// reproduce; zeroed here so an under-writing handler cannot leak stack
// bytes into a caller that declared more words than the handler filled.
let mut local_intout = [0i16; O_SIZE];
```

Das ist **kein Soll**: ein konformes Programm liest nur die Wörter, die die
Funktion definiert. Der Beweis, dass es wirklich Stapelrest ist und nicht
Zufall, steht in den Werten selbst — sie sind **wiedererkennbar** die Antwort
des jeweils vorangegangenen Falls:

| Fälle | Opcode | die nicht beschriebenen Wörter | EmuTOS-Rest stammt aus |
|---|---|---|---|
| 275 | 0 | `int_out[1..4]` = 8, 16, 19, 19 | `graf_handle` (`gl_wchar/hchar/wbox/hbox`) beim Start des Dumpers |
| 283 | 77 | `int_out[6]` = 1 | Stapel; `int_out[0..4]` stimmen auf beiden Seiten überein |
| 333 | 104/10 | `int_out[2], [4]` = 0x0218, 0xA000 | der `WF_SCREEN`-Fall davor |
| 335, 336, 397, 400 | 104 | `int_out[1..4]` (Absage, nichts geschrieben) | dito |
| 415, 418, 421 | 104/8,9,10 | `int_out[2..4]` = 20, 300, 200 | der `WF_FXYWH`-Fall davor |
| 427, 430 | 104/13,14 | `int_out[1..4]` = 31, 63, 296, 194 | der `WF_WXYWH`/`WF_FIRSTXYWH`-Fall davor |
| 433, 436 | 104/15,16 | `int_out[2..4]` | dito |
| 445, 448 | 104/21,99 | `int_out[1..4]` | der `WF_SCREEN`-Fall davor |
| 452 | 104/12 | `int_out[1..2]` = 31, 63 | siehe unten |
| 568–574 | 46 | `int_out[2]` = 63 | `objc_edit` schreibt nur `int_out[0]` und `OB_ODX = int_out[1]` |

Das sind **23 Fälle**: 275, 283, 333, 335, 336, 397, 400, 415, 418, 421, 427,
430, 433, 436, 445, 448, 452 und 568–574.

Zwei Feinheiten, die dabei zugunsten von NewTOS ausgehen und festgehalten
gehören:

* **Fall 452 (`WF_NEXTXYWH` am Listenende)** sieht wie ein Inhaltsfehler aus, ist
  aber keiner. `w_owns` beschreibt bei leerer Liste **nur** Breite und Höhe:
  ```c
  /* aes/gemwmlib.c:1230-1241 */
  while (po) { ... }
  poutwds->g_w = poutwds->g_h = 0;
  ```
  `g_x`/`g_y` bleiben unberührt. Die vertraglichen Wörter `int_out[3] = 0` und
  `int_out[4] = 0` **stimmen auf beiden Maschinen überein**; nur die beiden
  unbeschriebenen weichen ab. NewTOS hält das ausdrücklich so
  (`tos-api/src/aes/window.rs`, `Owns::Empty`-Zweig: „writing zeroes here would
  claim a contract EmuTOS does not have").
* **Fall 400 (`WF_KIND`)** bestätigt eine gelandete Reparatur: EmuTOS **und**
  NewTOS antworten `int_out[0] = 0`. Die frühere bewusste Abweichung D-053 /
  AUD-P5-014 ist weg (`tos-api/src/aes/window.rs`, `_ => { pb.set_ret(0); }`).
* Alle Fälle mit `OUT_LEN 5` zeigen `int_out[5]` und `int_out[6]` auf **beiden**
  Maschinen als `0xDEAD`-Gift: NewTOS respektiert `OUT_LEN` und schreibt
  nicht darüber hinaus (`Aespb::copy_intout_back`, `tos-api/src/aes/mod.rs:132-139`).

### S3-b · `evnt_dclick`-Rücklesen (Fälle 343, 344): unterschiedliche `.INF`-Vorgabe

| | |
|---|---|
| **EmuTOS** | `int_out[0] = 2` |
| **NewTOS** | `int_out[0] = 3` |
| **Verdikt** | **CONFIRMED als Messkonfundierung** — auf beiden Maschinen nachgefahren |

`gl_dcindex` ist ein `static WORD` (`aes/gemevlib.c:34`), und `ev_dclick`
liefert ihn unverändert zurück, wenn `setit == 0` (`:300-309`). Gesetzt wird er
an drei Stellen:

1. `ev_dclick(3, TRUE)` beim AES-Start (`aes/geminit.c:708`),
2. `ev_dclick(env & 0x07, TRUE)` aus der `#E`-Zeile der `.INF`
   (`aes/geminit.c:425`),
3. `evnt_dclick(G.g_cdclkpref, TRUE)` durch den Desktop (`desk/deskmain.c:1460`),
   dessen Wert aus derselben `#E`-Zeile stammt (`desk/deskapp.c:921`).

Die beiden Läufe hatten **verschiedene Vorgaben**:

* TT: keine `.INF` im Arbeitsverzeichnis → EmuDesk nimmt seine eingebaute
  Vorgabe `"#E 1A E0 00 00 60"` (`desk/deskapp.c:173`); `0x1A & 0x07 = 2`.
  Der Desktop lief vor dem Dumper und hat die 2 gesetzt.
* NewTOS: `C:\NEWDESK.INF` mit **nur** `#Z 01 C:\DUMPAES.NAP@` — keine
  `#E`-Zeile, und wegen des Autostarts lief der Desktop gar nicht erst. Es
  blieb bei der AES-Vorgabe 3.

**Nachgefahren statt behauptet** (Abschnitt 6): mit `#E 1D` (⇒ 5) antwortet
EmuTOS 5 **und** NewTOS 5. Damit ist belegt, dass der Wert der `#E`-Zeile folgt
und NewTOS' `process_inf2`-`#E`-Zweig
(`tos-api/src/aes/init.rs:376-378`) durchgängig funktioniert.

**Kein Defekt.** Ein Hinweis für künftige Tabellenläufe: beide Maschinen
brauchen dieselbe `.INF`-Vorgabe, sonst misst man die `.INF`.

### S3-c · Bilanz Schublade 3

23 (S3-a) + 2 (S3-b) = **26**. Die drei `WF_SCREEN`-Fälle sind bewusst **nicht**
hier, obwohl sie aussehen wie die anderen `wind_get`-Fälle: `WF_SCREEN` hat
einen eigenen Zweig, der alle vier Wörter **beschreibt** — dort ist die Null
eine Antwort, kein Rest.

---

## 5. Bezug zu den bestehenden Tickets

| Ticket | Wirkung dieser Messung |
|---|---|
| **[A50](../../TASK/A50.md)** (`rsrc_gaddr`/`saddr`-Ränder) | **bestätigt**, 25 Fälle. Aus „gelesen" wird „gemessen". Die Abwägung („unsere Fassung ist die robustere") bleibt gültig — Angleichen hiesse die Indexprüfung entfernen. |
| **[A42](../../TASK/A42.md)** (`wind_get(WF_NAME/WF_INFO)`) | **weder bestätigt noch widerlegt** — **0 von 20** `wind_get`-Fällen betreffen ihn. Die Tabelle schliesst die Felder 2 und 3 **konstruktiv** aus („Named and NOT here", Tabellenkopf), weil sie einen Zeiger führen und der Lader sie abweist. Der Defekt selbst ist aus der Quelle unverändert vorhanden: `tos-api/src/aes/window.rs:2740-2754` antwortet mit `1` + Text, `aes/gemwmlib.c:1590-1592` mit `FALSE`. A42 braucht einen eigenen Lauf (Hosttest oder eine Tabelle, die den Zeiger führen darf). |
| Inventar-Befund **A-6** (`WF_SCREEN`) | von „bewusste Abweichung, gelesen" auf **Defekt, gemessen**. Braucht ein Ticket **und** eine Korrektur des Inventar-Status (zu [A53](../../TASK/A53.md)). |
| Inventar-Befund **A-4** | = A50, siehe oben. |
| Versionsgatter `acb7f3b4` | bestätigt und zusätzlich abgenommen: NewTOS' `WF_OWNER` stimmt Wort für Wort mit der gepinnten Quelle. |
| Versionsgatter `4b0d9518` | von dieser Tabelle nicht berührt (`objc_draw` wird nicht ausgelöst) — wie im Tabellenkopf angekündigt. Der zugehörige Defekt bleibt [A46](../../TASK/A46.md). |
| frühere Reparatur **D-053 / AUD-P5-014** (`WF_KIND`) | **abgenommen** — Fall 400 stimmt auf beiden Maschinen. |

### Tickets, die es braucht (anzulegen vom Anwender)

1. **`graf_mkstate`/VDI-Cursor startet auf `w/2, h/2` statt `(w−1)/2, (h−1)/2`** —
   `tos-api/src/vdi/mouse.rs:321-322` gegen `vdi/vdi_mouse.c:582-583` +
   `vdi/vdi_defs.h:94` + `vdi/vdi_control.c:203-204`. P2.
2. **`wind_get(WF_SCREEN)` liefert Null-Adresse und Null-Länge** —
   `tos-api/src/aes/window.rs:2672-2679` gegen `aes/gemwmlib.c:1563-1565` +
   `aes/gemgsxif.c:96-129`. Braucht einen echten persistenten Menü-/Alarm-Puffer.
   P1 (Regel-2-Nähe: fester Wert, behauptet Erfolg).
3. **`objc_add` liefert 0 statt 1** — `tos-api/src/aes/object.rs:1071-1081`
   gegen `aes/gemsuper.c:84,204-206` + `aes/gemoblib.c:1121-1141`. P2.
4. **`rsrc_gaddr` liefert beim unbekannten Typ `addr_out = 0` statt `−1`** —
   `tos-api/src/aes/resource.rs:1586` gegen `aes/gemrslib.c:170-172,717-721`.
   P2, oder als zweiter Punkt an A50.
5. **`ap_id` des gestarteten Programms ist 1 statt 0** —
   `tos-api/src/aes/appl.rs:191-199` gegen `aes/gemaplib.c:54-63` +
   `aes/geminit.c:875-876,884-888`. P2; bitte zusammen mit der Frage
   entscheiden, welche pid NewTOS als Absender in `msg[1]` einträgt.
6. **Dokumentationskorrektur** (an A53): Inventar-Befund A-6 ist kein
   „bewusste Abweichung" mehr.

---

## 6. Was ich ausgeführt habe

Alles hier Behauptete ist entweder aus der gepinnten Quelle zitiert oder
gefahren. Gefahren wurde:

| Lauf | Aufbau | Ergebnis |
|---|---|---|
| **Hatari A** (Kontrolle) | EmuTOS 1.4 / TT / ttmed / VGA, Arbeitsverzeichnis **ohne** `.INF`, eigene 4-Fall-Probetabelle (CRC `73A61D28`) | `evnt_dclick` → **2**, `graf_mkstate` → **319/239**, `graf_handle` → 8/16/19/19. Reproduziert die r4-Messung exakt — die Probe ist tragfähig. |
| **Hatari B** | dasselbe, aber `EMUDESK.INF` mit `#E 1D E0 00 00 60` | `evnt_dclick` → **5**. Der Wert folgt der `.INF`. |
| **NewTOS / QEMU raspi4b** | `kernel8.img` SHA `4a67184108a65c33988699f6ff13dbda9155686fb97e40cf8e82bb6ca83a3b20` — **byte-identisch** mit der r4-Messung; `DUMPAES.NAP` SHA `5889e9bb…` ebenso; `NEWDESK.INF` = `#R 02` + `#Z 01 C:\DUMPAES.NAP@` + `#E 1D E0 00 00 60` | `evnt_dclick` → **5** (wie EmuTOS), `graf_mkstate` → **320/240**, `graf_handle` → 8/16/19/19. Kein Panic, kein Fault, Desktop kam danach hoch. |
| **Hosttest** | `cargo test -p tos-api --test aes_rsrc_tedinfo_txtlen` | 3 von 3 grün; `rsrc_gaddr(R_TEDINFO, i)` und `rsrc_gaddr(R_TREE, 0)` liefern mit **geladener** Resource `1`. `rsrc_gaddr` ist also nicht grundsätzlich kaputt — die 25 Nullen der Messung sind der Schutz, nicht ein Totalausfall. |

Die Probetabelle (4 Fälle, von `casecheck --aes` mit Exit 0 angenommen):

```
# probe: evnt_dclick read-back under a controlled EMUDESK.INF #E line.
!reset	wind_new
26	2	1	0	0	0,0	evnt_dclick read, rate 0 -- the gl_dcindex in force
26	2	1	0	0	4,0	evnt_dclick read, rate 4 -- same answer
79	0	5	0	0	-	graf_mkstate -- the initial mouse position
77	0	5	0	0	-	graf_handle -- device parameters for the record
```

Ausserdem nachgeprüft statt übernommen: die Auswahl der 64 (eigener Parser),
die Opcode-Verteilung, die Anzahl der AES-Commits seit `VERSION_1_4`, und für
jeden Schublade-1-Befund die NewTOS-Stelle im aktuellen Baum.

---

## 7. Was unentschieden bleibt

Ehrlich benannt:

1. **A42 ist von dieser Runde nicht berührt.** Kein Fall führt `WF_NAME` oder
   `WF_INFO`. Der Defekt steht weiter aus der Quelle, aber **gemessen ist er
   nicht** — und ich behaupte nicht, ihn gemessen zu haben.
2. **Das Fehlerbild von D-6 (`ap_id`) ist hergeleitet, nicht gefahren.** Dass ein
   reales Programm über `msg[1] == gl_apid` stolpert, ist plausibel und aus
   `aes/gemwmlib.c:855` begründet — ich habe kein Programm laufen lassen, das
   es tut.
3. **`rsrc_gaddr` mit geladener Resource ist nur für zwei Selektoren gefahren**
   (`R_TEDINFO`, `R_TREE`, über den Hosttest). Für die übrigen 15 ist mein
   Urteil aus dem Code hergeleitet. Insbesondere ungefahren: eine Resource
   **ohne** BITBLK-/ICONBLK-Abschnitt, wo NewTOS' `target_base == 0`-Prüfung
   `FALSE` liefert und EmuTOS `TRUE`.
4. **D-2 (`WF_SCREEN`) ist kein Klick- oder Screendump-Befund**, sondern ein
   Wertbefund — aber ob ein *konkretes* portiertes Programm (QED, TeraDesk) den
   Puffer anfordert, habe ich nicht gesucht. Die Einstufung als Defekt stützt
   sich auf die dokumentierte API, nicht auf einen betroffenen Anwender.
5. **Die Vorbedingung von D-4** (keine Resource geladen) ist für ein konformes
   Programm unerreichbar. Ich stufe die 25 Fälle trotzdem als Abweichung ein,
   weil der Rückgabecode je Typ ein geräteunabhängiges Soll hat — die
   *Schwere* ist damit aber gering, und A50s Abwägung bleibt die richtige.

---

## 8. Beobachtungen ausserhalb des Auftrags

Je eine Zeile, nicht verfolgt:

* NewTOS trägt in `msg[1]` die **eigene** pid des Aufrufers ein
  (`tos-api/src/aes/window.rs:1500,3842,3971`), wo EmuTOS die pid des
  **Absenders** einträgt — beim Control Manager also 1 (`aes/gemwmlib.c:855`).
* `tos-api/src/aes/event.rs:360` bildet `ev_dclick(3, TRUE)` als
  Startwert eines `AtomicI16` ab statt als Aufruf; funktional gleich, aber der
  Zeitpunkt relativ zu `gl_ticktime` (`aes/geminit.c:703-708`) existiert bei uns
  nicht — `gl_dclick` (die Tickzahl) wird nirgends berechnet.
* Der Lader lässt `scrp_read` (80) und `scrp_write` (81) durch, obwohl sie den
  vergifteten Zeiger dereferenzieren — die Tabelle hält sie nur durch Verzicht
  draussen, nicht durch den Schutz (`tosdump/core/cases.c`, `aes_opcodes[]`).

---

## 9. Aufräumbestätigung

* **Kein Produktcode angefasst.** Keine `.rs`-Datei geändert, keine `.c`-Datei,
  keine Tabelle, kein Ticket.
* **Kein Messartefakt verändert**: `tosdump/cases/`, `tosdump/reference/` und
  alle `measurement-*`-Verzeichnisse sind unberührt; die r4-Messung wurde nur
  gelesen.
* Probetabelle, Probe-Abbild, die drei Probeläufe und der Dekoder liegen
  ausschliesslich im Scratch-Verzeichnis der Sitzung.
* `kernel8.img` im Projektwurzelverzeichnis ist der gitignorierte Regelausgang
  des Bauschritts (`.gitignore:8`) und **byte-identisch** mit dem der
  r4-Messung; er wurde für den QEMU-Probelauf erzeugt und stehen gelassen, weil
  ihn andere Stränge erwarten.
* Einzige neue Datei im Arbeitsbaum: **dieses Dokument**, exklusiv angelegt
  (vorher auf Existenz geprüft, `set -o noclobber`).
* Kein `git checkout` / `clean` / `stash` / `restore` / `reset`, kein Commit.
* Nach jedem Lauf `pgrep -f 'qemu-system-aarch64|Hatari'` geprüft: kein Prozess
  übrig. Keine Hardware berührt, nichts geflasht, nichts ausgeworfen;
  `/Volumes/BOOT`, `/Volumes/BOOT 1`, `/Volumes/TOOLS` nicht angefasst.

---

## Anhang — die 64 Fälle einzeln

`E`/`N` zeigen nur die **abweichenden** Wörter. Schublade 1 = NewTOS ist
falsch, 2 = Versionsdelta, 3 = Geräteparameter oder kein Soll.

| Fall | Op | was der Fall prüft | EmuTOS | NewTOS | Schu. | Beleg |
|---|---|---|---|---|---|---|
| 275 | 0 | opcode 0 with all four counts and four inputs: marshalled in, nothing written back | `[1]=8` `[2]=16` `[3]=19` `[4]=19` | `[1]=0` `[2]=0` `[3]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 283 | 77 | graf_handle OUT_LEN 7: two words past what the call writes, from xif's own stack | `[6]=1` | `[6]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 286 | 79 | graf_mkstate: the four words gr_mkstate writes | `[1]=319` `[2]=239` | `[1]=320` `[2]=240` | **1** | **D-1** `mouse.rs:321-322` vs `vdi_mouse.c:582-583` |
| 331 | 104 | WF_SCREEN OUT_LEN 5: the two LONGs gsx_mret writes | `[1]=256` `[2]=536` `[4]=-24576` | `[1]=0` `[2]=0` `[4]=0` | **1** | **D-2** `window.rs:2672-2679` vs `gemwmlib.c:1563-1565` |
| 332 | 104 | WF_SCREEN on an invalid handle: no handle needed, so it must still answer | `[1]=256` `[2]=536` `[4]=-24576` | `[1]=0` `[2]=0` `[4]=0` | **1** | **D-2** `window.rs:2672-2679` vs `gemwmlib.c:1563-1565` |
| 333 | 104 | WF_TOP on an invalid handle: likewise | `[2]=536` `[4]=-24576` | `[2]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 334 | 104 | WF_OWNER on the desktop handle, OUT_LEN 5 -- VERSION DELTA: absent in 1.4 | `[0]=0` `[2]=536` `[3]=0` `[4]=-24576` | `[0]=1` `[2]=0` `[3]=-1` `[4]=-1` | **2** | `acb7f3b4`, `gemwmlib.c:1578-1589` |
| 335 | 104 | WF_WXYWH on an invalid handle: the pre-check must refuse before the switch | `[2]=536` `[4]=-24576` | `[2]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 336 | 104 | WF_HSLIDE on a handle past NUM_WIN | `[2]=536` `[4]=-24576` | `[2]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 343 | 26 | evnt_dclick read, rate 0: the rate word is ignored when setit is 0 | `[0]=2` | `[0]=3` | **3** | **S3-b** `.INF`-`#E`, `deskapp.c:173` / `geminit.c:425` |
| 344 | 26 | evnt_dclick read, rate 4: same answer, proving the rate was ignored | `[0]=2` | `[0]=3` | **3** | **S3-b** `.INF`-`#E`, `deskapp.c:173` / `geminit.c:425` |
| 397 | 104 | wind_get field 0: below the first defined field, must reach default | `[2]=536` `[4]=-24576` | `[2]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 400 | 104 | wind_get WF_KIND: defined in the header, answered by neither wm_get nor wm_set | `[2]=536` `[4]=-24576` | `[2]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 415 | 104 | wind_get WF_HSLIDE: w_setup leaves the slider at the left | `[2]=20` `[3]=300` `[4]=200` | `[2]=0` `[3]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 418 | 104 | wind_get WF_VSLIDE | `[2]=20` `[3]=300` `[4]=200` | `[2]=0` `[3]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 421 | 104 | wind_get WF_TOP: the one open window is the top one | `[2]=20` `[3]=300` `[4]=200` | `[2]=0` `[3]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 427 | 104 | wind_get field 13: the gap in the WF_ numbering | `[1]=31` `[2]=63` `[3]=296` `[4]=194` | `[1]=0` `[2]=0` `[3]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 430 | 104 | wind_get WF_NEWDESK: settable but not gettable -- wm_get has no arm | `[1]=31` `[2]=63` `[3]=296` `[4]=194` | `[1]=0` `[2]=0` `[3]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 433 | 104 | wind_get WF_HSLSIZ | `[2]=63` `[3]=296` `[4]=194` | `[2]=0` `[3]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 436 | 104 | wind_get WF_VSLSIZ | `[2]=63` `[3]=296` `[4]=194` | `[2]=0` `[3]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 439 | 104 | wind_get WF_SCREEN on a real window: still the menu/alert save buffer | `[1]=256` `[2]=536` `[4]=-24576` | `[1]=0` `[2]=0` `[4]=0` | **1** | **D-2** `window.rs:2672-2679` vs `gemwmlib.c:1563-1565` |
| 442 | 104 | wind_get WF_OWNER on an open window -- VERSION DELTA: absent in 1.4 | `[0]=0` `[1]=256` `[2]=536` `[3]=0` `[4]=-24576` | `[0]=1` `[1]=1` `[2]=1` `[3]=1` `[4]=1` | **2** | `acb7f3b4`, `gemwmlib.c:1578-1589` |
| 445 | 104 | wind_get field 21: past the last defined field | `[1]=256` `[2]=536` `[4]=-24576` | `[1]=0` `[2]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 448 | 104 | wind_get field 99: far past it | `[1]=256` `[2]=536` `[4]=-24576` | `[1]=0` `[2]=0` `[4]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 452 | 104 | wind_get WF_NEXTXYWH after a FIRST on the same handle: the end of a one-piece list | `[1]=31` `[2]=63` | `[1]=0` `[2]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 550 | 40 | objc_add(NIL, NIL): ob_add's guard refuses before the first tree access | `[0]=1` | `[0]=0` | **1** | **D-3** `object.rs:1071-1081` vs `gemsuper.c:84,204-206` |
| 551 | 40 | objc_add(NIL, ROOT): parent NIL alone is enough | `[0]=1` | `[0]=0` | **1** | **D-3** `object.rs:1071-1081` vs `gemsuper.c:84,204-206` |
| 552 | 40 | objc_add(ROOT, NIL): child NIL alone is enough | `[0]=1` | `[0]=0` | **1** | **D-3** `object.rs:1071-1081` vs `gemsuper.c:84,204-206` |
| 568 | 46 | objc_edit EDSTART on object 1: the kind arm returns first, tree untouched | `[2]=63` | `[2]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 569 | 46 | objc_edit EDSTART on the ROOT | `[2]=63` | `[2]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 570 | 46 | objc_edit EDINIT on the ROOT: obj <= 0 returns before the TEDINFO | `[2]=63` | `[2]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 571 | 46 | objc_edit EDCHAR 'A' on the ROOT | `[2]=63` | `[2]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 572 | 46 | objc_edit EDEND on the ROOT | `[2]=63` | `[2]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 573 | 46 | objc_edit kind 99: no case arm, the function falls through to TRUE | `[2]=63` | `[2]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 574 | 46 | objc_edit EDINIT on a negative object | `[2]=63` | `[2]=0` | **3** | **S3-a** `gemsuper.c:429-432,443-445` vs `mod.rs:259-263` |
| 598 | 112 | rsrc_gaddr type 0 index 0 -- R_TREE: ap_ptree[index], and ap_ptree is NULL on both machines | `[0]=1` `addr=0x602E0206` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 599 | 112 | rsrc_gaddr type 1 index 0 -- R_OBJECT | `[0]=1` `addr=0x00000206` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 600 | 112 | rsrc_gaddr type 2 index 0 -- R_TEDINFO -- identity pair with R_TEPTEXT 8 | `[0]=1` `addr=0x000000E0` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 601 | 112 | rsrc_gaddr type 3 index 0 -- R_ICONBLK -- identity pair with R_IBPMASK 11 | `[0]=1` `addr=0x00000030` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 602 | 112 | rsrc_gaddr type 4 index 0 -- R_BITBLK -- identity pair with R_BIPDATA 14 | `[0]=1` `addr=0x000000E0` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 603 | 112 | rsrc_gaddr type 5 index 0 -- R_STRING: the one type that dereferences the slot it computes | `[0]=1` `addr=0x0D000001` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 604 | 112 | rsrc_gaddr type 6 index 0 -- R_IMAGEDATA: likewise | `[0]=1` `addr=0x00E007E0` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 605 | 112 | rsrc_gaddr type 7 index 0 -- R_OBSPEC: &obj->ob_spec, a sub-pointer of R_OBJECT | `[0]=1` `addr=0x00000212` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 606 | 112 | rsrc_gaddr type 8 index 0 -- R_TEPTEXT -- must equal R_TEDINFO 2, same case label | `[0]=1` `addr=0x000000E0` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 607 | 112 | rsrc_gaddr type 9 index 0 -- R_TEPTMPLT: &tedinfo->te_ptmplt | `[0]=1` `addr=0x000000E4` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 608 | 112 | rsrc_gaddr type 10 index 0 -- R_TEPVALID: &tedinfo->te_pvalid | `[0]=1` `addr=0x000000E8` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 609 | 112 | rsrc_gaddr type 11 index 0 -- R_IBPMASK -- must equal R_ICONBLK 3, same case label | `[0]=1` `addr=0x00000030` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 610 | 112 | rsrc_gaddr type 12 index 0 -- R_IBPDATA: &iconblk->ib_pdata | `[0]=1` `addr=0x00000034` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 611 | 112 | rsrc_gaddr type 13 index 0 -- R_IBPTEXT: &iconblk->ib_ptext | `[0]=1` `addr=0x00000038` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 612 | 112 | rsrc_gaddr type 14 index 0 -- R_BIPDATA -- must equal R_BITBLK 4, same case label | `[0]=1` `addr=0x000000E0` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 613 | 112 | rsrc_gaddr type 15 index 0 -- R_FRSTR: the free-string POINTER, not the string | `[0]=1` `addr=0x00000774` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 614 | 112 | rsrc_gaddr type 16 index 0 -- R_FRIMG: the free-image pointer | `[0]=1` `addr=0x000000E0` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 616 | 112 | rsrc_gaddr type 2 index 1 -- R_TEDINFO, one record on from index 0 | `[0]=1` `addr=0x000000FC` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 617 | 112 | rsrc_gaddr type 8 index 1 -- R_TEPTEXT, one record on from index 0 | `[0]=1` `addr=0x000000FC` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 618 | 112 | rsrc_gaddr type 3 index 1 -- R_ICONBLK, one record on from index 0 | `[0]=1` `addr=0x00000052` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 619 | 112 | rsrc_gaddr type 11 index 1 -- R_IBPMASK, one record on from index 0 | `[0]=1` `addr=0x00000052` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 620 | 112 | rsrc_gaddr type 4 index 1 -- R_BITBLK, one record on from index 0 | `[0]=1` `addr=0x000000EE` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 621 | 112 | rsrc_gaddr type 14 index 1 -- R_BIPDATA, one record on from index 0 | `[0]=1` `addr=0x000000EE` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 623 | 112 | rsrc_gaddr type 17: one past the last defined R_* type | `addr=0xFFFFFFFF` | `addr=0x00000000` | **1** | **D-5** `resource.rs:1586` vs `gemrslib.c:170-172` |
| 624 | 112 | rsrc_gaddr type 99 | `addr=0xFFFFFFFF` | `addr=0x00000000` | **1** | **D-5** `resource.rs:1586` vs `gemrslib.c:170-172` |
| 625 | 112 | rsrc_gaddr type -1: rstype is a UWORD, so this is 65535 | `addr=0xFFFFFFFF` | `addr=0x00000000` | **1** | **D-5** `resource.rs:1586` vs `gemrslib.c:170-172` |
| 626 | 112 | rsrc_gaddr R_OBJECT index 1000: EmuTOS does not range-check the index | `[0]=1` `addr=0x00005FC6` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 627 | 112 | rsrc_gaddr R_OBJECT with n_addrout 0: xif writes addrout[0] regardless | `[0]=1` `addr=0x00000206` | `[0]=0` `addr=0x00000000` | **1** | **D-4** `resource.rs:1430-1442` vs `gemrslib.c:717-722` |
| 644 | 10 | appl_init re-entry: ap_init sets AP_OPEN on a process that already has it | `[0]=0` `[1]=7` | `[0]=1` `[1]=0` | **1** | **D-6** `appl.rs:191-199` vs `gemaplib.c:54-63` |
