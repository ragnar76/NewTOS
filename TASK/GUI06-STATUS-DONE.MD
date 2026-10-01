# GUI06 — Status: **DONE**, mit zwei benannten offenen Punkten

Basis: `desk-rebuild` @ `9731f16`. Zwischen dem im Auftrag genannten `d7ecec5`
und `9731f16` liegt **kein Quelltext**, nur drei TASK-Dokumente
(`git diff --stat d7ecec5 HEAD` = `TASK/PORT02.md`, `TASK/SDL01.md`,
`TASK/SND01.md`) — der gemessene „Vorher"-Stand ist also wortgleich `d7ecec5`.

Messverzeichnis: `measurements/measurement-2026-09-28-gui06/` (kein `*.img`).
Der Fix als Diff: `measurements/measurement-2026-09-28-gui06/method/gui06-fix.diff`.

**Was NICHT erledigt ist, vorweg:**

1. Die Abnahme auf der echten Pi steht aus. Ich fahre keine Hardware; die
   Vorhersage, gegen die zu messen ist, steht in `NUMBERS.TXT` §4.
2. `fun_desk2desk` (Ziehen zwischen zwei Desktop-Symbolen) behält sein
   `redraw_desktop()` — absichtlich, mit Begründung unten. Es ist **nicht**
   die gemessene Geste.

---

## 1. Schritt 1 des Tickets: die Zuordnung. Kein Fix davor.

Die Sonde hatte beide Zähler bereits, und die vorhandene Pi-Messung
(`measurements/measurement-2026-09-25-fs04/raw/pi04-FS04.TXT`, 13 Blöcke bei 1920×1080)
ist ein natürliches Experiment: zwischen den Blöcken schwankt die Zahl der
`dc cvac` um den Faktor **1,98**, die Neuzeichenzeit um **1,07** — und zwar
in die falsche Richtung.

| | |
|---|---|
| Block 7 | 4 892 879 Cachezeilen → 2×redraw = 226 917 µs |
| Block 10 | 9 667 213 Cachezeilen → 2×redraw = 227 676 µs |
| | **+98 % Cachepflege → +0,3 % Neuzeichenzeit** |

Regression über die 12 Blöcke 3–14:

* Neuzeichenzeit gegen `fb.clean_lines`: Steigung **−1,50 ns/Zeile**, r = −0,48.
  Negativ. Die Cachepflege trägt die Neuzeichenzeit nicht.
* `Rest` (= wall − 2×redraw − gemdos) gegen `fb.clean_lines`: Steigung
  **+1,32 ns/Zeile**, r = +0,56. Das ist der echte Grenzpreis einer `dc cvac`
  auf der Pi 4 — rund **zwei Zyklen** bei 1,5 GHz, also der Preis für eine
  Zeile, die **nicht schmutzig** ist.

Und genau das sind fast alle: der Bildschirm hat 129 600 Cachezeilen, die
Pflege läuft über 4,9–9,7 Millionen. Jede Zeile wird beim ersten Mal
zurückgeschrieben und danach 40–70-mal nachgeschlagen und für sauber
befunden. **Die 313–618 MB des Tickets sind echt — aber es sind zum
allergrößten Teil Nachschläge, keine Rückschreibungen.**

### Die Antwort auf Schritt 1

| Anteil | an den ~230 ms | absolut |
|---|---|---|
| **1 — zwei Vollbild-Neuzeichnen** | **≥ 95 %** | 208–225 ms |
| **2 — Spannenpflege (`dc cvac`)** | **≤ 5 %** | 6–13 ms |

Die Obergrenze für Anteil 2 ist großzügig gerechnet (alle Zeilen des ganzen
Fallenlassens den beiden Neuzeichnen zugeschlagen: 9 667 213 × 1,32 ns =
12,8 ms von 219–234 ms). Mit dem Median sind es 8,6 ms = 3,7 %.

Das halbiert die Arbeit, wie das Ticket hoffte — die Reihenfolge ist
entschieden: der Fix gehört an die **Zahl der Neuzeichnen**.

---

## 2. Schritt 2, Anteil 1: warum zwei Vollbild-Neuzeichnen

**EmuTOS löst nach einem Kopieren durch Ziehen null Vollbild-Neuzeichnen
aus.** Es löst genau **ein `WM_REDRAW` je neu aufgebautem Fenster** aus, über
dessen eigenes Rechteck:

```c
/* desk/deskfun.c:133-143 */
static void rebuild_window(WNODE *pwin)
{
    pn_active(&pwin->w_pnode, TRUE);
    desk_verify(pwin->w_id, TRUE);
    win_sinfo(pwin, FALSE);
    wind_get_grect(pwin->w_id, WF_WXYWH, &gr);
    fun_msg(WM_REDRAW, pwin->w_id, gr.g_x, gr.g_y, gr.g_w, gr.g_h);
}
```

`fun_msg` (`deskfun.c:100-113`) füllt `G.g_rmsg` und ruft sofort `hndl_msg()`,
dessen `WM_REDRAW`-Arm `do_wredraw(handle, &G.g_rmsg[4])` ist
(`deskmain.c:1268-1272`). Synchron, ein Fensterrechteck, sonst nichts.

Die vier Arme von `fun_drag`:

| EmuTOS | was es tut |
|---|---|
| `fun_win2desk` (`deskfun.c:1129-1130`) | `fun_rebld(src)` — sonst nichts |
| `fun_win2win` (`deskfun.c:986-999`) | `fun_rebld(src)` (nur MOVE), `fun_rebld(dst)` |
| `fun_desk2win` (`deskfun.c:1253-1254`) | `if (copied) fun_rebld(dst)` |
| `fun_desk2desk` (`deskfun.c:1260-1298`) | gar nichts; `desk_clear(DESKWH)` in `fun_file2any` (`:1187`) |

Der Bereich, den der COPY-FILE(S)-Kasten verdeckt hatte, ist da längst
wiederhergestellt — von `end_dialog` (`deskinf.c:363-378`), dessen
`form_dial(FMD_FINISH)` den Hintergrund über **seinem** Rechteck neu malt
und den betroffenen Fenstern ein `WM_REDRAW` schickt (`gemfmlib.c:375-379`).
Diese Kette gibt es in NewTOS vollständig und korrekt (`desk/inf.rs:2216`,
`tos-api/src/aes/window.rs:4755` `redraw_dialog_area`).

NewTOS hatte an genau diesen Stellen `redraw::redraw_desktop()` — einen
Vollbild-Verbund aus `form_dial(FMD_FINISH)` über den **ganzen** Bildschirm
+ `win_shwall` über alle Fensterinhalte + `install_menu_bar()`. Zweimal je
Fallenlassen: einmal in `rebuild_window` (`dir.rs:2300`), einmal im Aufrufer
(`dir.rs:2017`). Die Dokumentation von `rebuild_window` zitierte die richtige
EmuTOS-Zeile und tat dann etwas anderes.

### Geändert

`kernel/src/desk/dir.rs`

* `rebuild_window()` — `redraw_desktop()` → `wind_get_grect(handle, WF_WXYWH)`
  + `redraw::do_wredraw(handle, gr)` (`deskfun.c:141-142`)
* `fun_win2desk()` — `redraw_desktop()` entfernt (`deskfun.c:1129-1130`)
* `fun_win2win()` — `redraw_desktop()` entfernt (`deskfun.c:986-999`)
* `fun_desk2win()` — `redraw_desktop()` entfernt (`deskfun.c:1253-1254`)

---

## 3. Schritt 2, Anteil 2: warum die einschließende Spanne

`VdiScreen::clean` pflegte eine einzige zusammenhängende Spanne
`[base+(y0·stride+x0)·4 … base+(y1·stride+x1)·4+4)`. Für ein 2 Pixel breites,
401 Zeilen hohes Primitiv auf einer 1920-Pixel-Zeile sind das
400·7680+8 = **3 072 008 Byte statt 3 208** — **48 001 `dc cvac` statt
höchstens 802**. Genau diese zwei Zahlen stehen in `pi04-FS04.TXT` als
`fb.clean max/call` und `fb.clean_lines max/call` (Blöcke 10–14).

Die Frage des Tickets — „ob zeilenweise Pflege hier richtig ist, entscheidet
die Schreibgeometrie des jeweiligen Primitivs" — gegen alle acht Aufrufer
geprüft:

| Aufrufer | geschriebene Menge |
|---|---|
| `fill_rect` (`vdi_backend.rs:533`) | genau `[x0..=x1] × [y0..=y1]` |
| `abline`, waagrecht (`:592`) | eine Zeile, `[xlo..=xhi]` |
| `abline`, allgemein (`:643`) | Bresenham **innerhalb** des Hüllrechtecks |
| `fill_pattern` (`:674`) | das Rechteck (Teilmenge bei Transparent/XOR) |
| `draw_glyph_sized` (`:723`) | genau die Zelle |
| `blit_mono` (`:763`) | genau das Rechteck |
| `copy_rect` (`:813`) | genau das Zielrechteck (Teilmenge bei Zeilensprung) |
| `flush_rect` (`:850`) | das Rechteck des Aufrufers |

**Kein Aufrufer schreibt außerhalb von `[x0..=x1] × [y0..=y1]`.** Zeilenweise
Pflege deckt damit jedes geschriebene Byte ab — die Bedingung ist erfüllt.

### Geändert

`kernel/src/arch/aarch64/mmu.rs` — neu: `clean_dcache_rect()` /
`clean_invalidate_dcache_rect()`: `rows` Läufe zu `row_bytes` im Abstand
`stride_bytes`, und **ein** `dsb sy` für den ganzen Lauf. Ein `dsb` je Zeile
wäre teurer als das, was er spart; `dc`-Operationen brauchen untereinander
keine Schranke (ARM ARM B2.4).

`kernel/src/vdi_backend.rs` — `clean()` rechnet `row_bytes`/`rows` und ruft
die neuen Funktionen.

**Was die zeilenweise Pflege nicht mehr tut** und warum das richtig ist: sie
macht die Lücken zwischen den Zeilen nicht mehr ungültig, wenn ein
EL0-Prozess einen Non-Cacheable-Alias hält (`direct_screen_active`). Eine
Zeile kann nur schmutzig sein, wenn der Kernel sie geschrieben hat; jeder
Schreibvorgang geht durch eines der acht Primitive oben; jedes davon pflegt
danach genau seine Zeilen. Die Lücken schreibt niemand.

---

## 4. Vorher/Nachher aus derselben Sonde

Dieselbe Geste wie FS04 (`method/gui06-gesture.actions`, wortgleich
übernommen), QEMU raspi4b, 640×480, vierpartitioniertes Kartenabbild.
**Drei** Stände, damit die Ersparnis aufteilbar ist — `midclean` ist aus
`after` gebaut, indem genau eine Datei (`desk/dir.rs`) durch die
`before`-Fassung ersetzt wurde.

| | `before` | `midclean` (nur Anteil 2) | `after` (beides) |
|---|---|---|---|
| **wall** | **23 825 µs** | **13 000 µs** | **8 598 µs** |
| `desk.redraw_desk` | n=2, 14 094 µs | n=2, 6 238 µs | **n=0** |
| `desk.rebuild_win` | 10 564 µs | 4 474 µs | 2 894 µs |
| `desk.do_wredraw` | n=1, 1 335 µs | n=1, 709 µs | n=2, 2 157 µs |
| `fb.clean` Aufrufe | 2 310 | 2 310 | 1 332 |
| `fb.clean` Bytes | 87 080 336 | 4 806 364 | 1 964 080 |
| `fb.clean_lines` | 1 362 715 | 106 321 | **44 839** |

Und bei 1920×1080 (Treiberbetrieb, 5× `redraw_desktop` auf dem Boot-Desktop —
die Zahl der `dc cvac` ist reine Geometrie und gilt auf der Pi genauso):

| je Neuzeichnen | Spanne | zeilenweise |
|---|---|---|
| Cachezeilen | 256 202 | **134 135** (−47,6 %) |
| Bytes | 16,39 MB | **8,52 MB** (−48,0 %) |

Auf dem **Boot**-Desktop ist die Ersparnis „nur" die Hälfte, weil das größte
Primitiv (der Hintergrund, 1061 × 1920 px) schon die volle Breite hat. Mit
einem **offenen Fenster** — der Lage, die die Pi gemessen hat — sind es
−92,2 % (640×480-Geste, Tabelle oben), weil dort die schmalen hohen Primitive
dazukommen. Der Pi-Bericht zeigt dieselbe Signatur.

---

## 5. Die Ehrlichkeit zur Übertragbarkeit

**QEMU ist für Anteil 2 kein Maßstab und übertreibt ihn um den Faktor 8.**
Aus dem 1920×1080-Lauf lässt sich QEMUs eigener Preis ablesen: 610 335
eingesparte Zeilen sparten 6 677 µs = **10,9 ns je `dc cvac`** in TCG, gegen
**1,32 ns** auf der Pi 4 (§1). Die QEMU-Halbierung der Neuzeichenzeit durch
den Zeilen-Fix (7 047 → 3 119 µs) wird auf der Pi **nicht** eintreten; dort
bleiben die ≤ 5 % aus §1.

**Was der Anwender auf der Pi sehen soll** (Rechnung, nicht Messung — die
Herleitung steht in `NUMBERS.TXT` §4): der Fix ersetzt zwei
`redraw_desktop` (110 ms je Stück) durch ein zusätzliches `do_wredraw` über
das Fensterrechteck (~17 ms, aus dem in QEMU gemessenen Verhältnis 0,153 —
das bei 1920×1080 eher kleiner wird, die Zahl ist also eine Obergrenze).

| Pi-Block | vorher | vorhergesagt |
|---|---|---|
| 7 (kein Kartenzugriff) | 240 ms | **~30 ms** |
| 14 (23,9 ms Kopie) | 267 ms | **~63 ms** |
| 10 (86,6 ms Kopie) | 340 ms | **~129 ms** |

Aus 267–340 ms werden **30–130 ms**, und was dann übrig bleibt, ist die
Karte, nicht mehr der Desktop. Sichtbar soll sein: das Fenster mit der neuen
Verzeichnisliste erscheint, ohne dass der ganze Bildschirm einmal durchblitzt
— der Desktop-Hintergrund, die Symbole und die Menüzeile werden gar nicht
mehr angefasst.

**Was QEMU nicht zeigen kann:** Cachepflege überhaupt (TCG hat keine Caches)
— dass zeilenweise Pflege keine schalen Bildpunkte hinterlässt, folgt aus der
Schreibgeometrie (§3), ist aber nur auf der Pi beobachtbar. Und die
1920×1080-**Zeiten**; boardunabhängig sind nur die **Zahlen** der Operationen.

---

## 6. Beweise

* **Bildpunktgleich, Geste (640×480, raspi4b):** `01-cwin-before.png` ==
  `02-cwin-after.png`; `03-afterdrop-before.png` == `04-afterdrop-midclean.png`
  == `05-afterdrop-after.png` (sha256 `eb42b444…22265628`).
* **Bildpunktgleich, Startbild, beide Boards, ohne Sonde:**
  `06-boot-rpi4-before.png` == `07-boot-rpi4-after.png`;
  `08-boot-qemuvirt-before.png` == `09-boot-qemuvirt-after.png`.
  0 `[EXC]` / 0 `[PANIC]` in allen vier Startprotokollen.
* **Dateisystem:** `fsck_msdos -n` auf C:, D:, E: beider Kartenabbilder nach
  der Geste — sauber, Dateizahlen identisch (52/7/4), also dieselbe Kopie.
* **Bau am gemeinsamen Baum, je frisch je Board:** `tos-api 64 / kernel 94`
  auf `board-qemu-virt` **und** `board-rpi4` — genau die Ausgangswerte.
* **clippy, beide Merkmalssätze:** `tos-api 80 / kernel 125` — genau die
  Ausgangswerte; keine Warnung zeigt in eine der drei berührten Dateien
  (die drei in `mmu.rs` sitzen in `map_device_block`, weit vor dem Einschub,
  und standen vorher schon da).
* **`cargo run -p xtask -- test`:** 961 / 0 / 1 — unverändert.
* **`sh tools/desk-drift-probe.sh`:** 32 — unverändert.

---

## 7. Was bewusst stehen geblieben ist

**`fun_desk2desk` (`dir.rs:2249`) behält sein `redraw_desktop()`.**

EmuTOS hat dort kein Neuzeichnen; das Aufheben der Symbolauswahl macht
`desk_clear(DESKWH)` in `fun_file2any` (`deskfun.c:1187`), und EmuTOS'
`desk_clear` **zeichnet**: `desksupp.c:143` → `act_allchg(…, FALSE)` →
`do_wredraw(wh, &a)` über die Vereinigung der geänderten Symbolrechtecke
(`deskact.c`, Schluss von `act_allchg`).

NewTOS' `desk_clear` (`desk/main.rs:2208`) hat diese zweite Hälfte **nicht**:
es ändert nur den Zustand und gibt `changed` zurück, das die meisten Aufrufer
wegwerfen (`main.rs:592` nach `fun_drag`, `dir.rs` in `fun_win2win`). Würde
man das `redraw_desktop()` hier entfernen, ohne `desk_clear` vorher seine
Zeichenhälfte zu geben, blieben die Laufwerkssymbole sichtbar ausgewählt
stehen.

`desk_clear` liegt über `desk_clear_all` (`deskmain.c:958`) auf dem
**Klickpfad — bei jedem Fensterwechsel**. Elf Tage vor dem Termin ist das der
falsche Ort für eine nicht erzwungene Änderung, und `fun_desk2desk` ist nicht
die gemessene Geste. Als eigener Befund weitergereicht:

> **Befund (nicht behoben):** `desk::main::desk_clear` fehlt die
> Zeichenhälfte von EmuTOS' `desk_clear` (`desksupp.c:143` →
> `act_allchg(…, FALSE)` → `do_wredraw(wh, &a)`). Solange sie fehlt, braucht
> `fun_desk2desk` seinen Vollbild-Verbund als Ersatz, und `desk_clear_all`
> (`main.rs:3223`) trägt einen zweiten Ersatz (`win_shwall()` +
> `do_wredraw(DESKWH, g_desk())`), der auf jedem Fensterwechsel läuft.

Ebenfalls nicht angefasst, weil außerhalb des Fallenlassens:
`dir.rs:1532` (`fun_del_desktop`), `dir.rs:1598` (`fun_selectall`),
`main.rs:2102/2281/2284`, `mod.rs:1046` (Startbild — dort ist der Verbund
richtig).

---

## 8. Auffälligkeit in den Pi-Zahlen, die nicht zu GUI06 gehört

Die Blöcke **1–2 jedes** Pi-Laufs zeichnen mit 46,6 bzw. 44,7 ms je
Neuzeichnen, ab Block 3 sind es 109–117 ms — bei **gleicher** Arbeit
(Block 2: 6 441 044 Cachezeilen, Block 3: 6 466 070). Das Verhältnis ist in
beiden Läufen 116,5/46,6 = 2,50 und 111,2/44,7 = 2,49. Die beiden
ARM-Taktraten der Pi 4 verhalten sich wie 1500/600 = **2,50**.

Das ist eine **Auffälligkeit, keine Feststellung** — prüfbar nur auf der Pi
(Mailbox `GET_CLOCK_RATE` / `GET_THROTTLED`). Wenn sie stimmt, läuft alles ab
Block 3 auf dem halben Takt, und die Hälfte der ~230 ms ist gar keine
Zeichenarbeit. **Für GUI06 ändert das nichts:** die Zuordnung auf die zwei
Anteile ist ein Verhältnis und taktunabhängig.

---

## 9. Berührte Dateien

```
kernel/src/desk/dir.rs            rebuild_window, fun_win2desk,
                                  fun_win2win, fun_desk2win
kernel/src/vdi_backend.rs         VdiScreen::clean
kernel/src/arch/aarch64/mmu.rs    clean_dcache_rect,
                                  clean_invalidate_dcache_rect  (neu)
kernel/src/drivers/bcm2711/       EINE Kommentarzeile: die Begruendung der
  framebuffer.rs                  cacheablen Abbildung nannte
                                  `clean_dcache_range`, das das VDI-Backend
                                  nicht mehr ruft.  Kein Code.
```

Nicht committet; die Arbeit liegt im Baum.
