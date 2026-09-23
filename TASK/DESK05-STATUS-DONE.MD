# DESK05 — STATUS: DONE (2026-09-23)

**Auftrag:** `Options → Desktop configuration...` nach EmuTOS bauen; Umfang
selbst aus der Quelle bestimmen, Abgrenzung zu DESK02 benennen.

Basis: Branch `desk-rebuild`, HEAD `5761465`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung: [`measurement-2026-09-23-desk05/`](../measurement-2026-09-23-desk05/).
Spezifikation: `docs/api/desktop/deskinf.md` (§`inf_conf`, DESK05) und
`docs/api/desktop/deskmain.md` (§ die Kuerzel-Maschinerie, DESK05).

---

## 1. Der Umfang, und woher er kommt

`CONFITEM` → `inf_conf()` (`deskmain.c:895-898`), hinter
`CONF_WITH_DESKTOP_CONFIG`. Das Flag steht **auf 1** in der Vorgabe
(`include/config.h:1774-1776`); die `0` bei `:349-351` gehoert zum
Minimal-/Nicht-Atari-Block und gilt fuer unser Falcon030/TOS4-Profil nicht.
`men_update` nennt `CONFITEM` in keiner `ILL_*`-Tabelle und in keinem
`menu_ienable`-Aufruf (`deskmain.c:143-160`, `:464-609`) — der Punkt ist
**immer** schwarz, genau wie `PREFITEM`.

`inf_conf` (`deskinf.c:1096-1246`) bearbeitet vier Dinge. Gebaut sind die
zwei mit einem lebenden Verbraucher, plus die beiden Anzeigen:

| Teil von `ADDESKCF` | Zustand | Grund |
|---|---|---|
| Menue-Kuerzel (`DCMNU*`) | **gebaut** | `lookup_ascii_shortcut` ist der Verbraucher, und den hat NewTOS |
| Funktionstasten (`DCFUN*`) | **gebaut** (Anzeiger) | liest `a_funkey` aus dem `NEWDESK.INF`-Modell, das NewTOS fuehrt |
| Freier Speicher (`DCFREMEM`) | **gebaut** | `Mxalloc(-1)`, neue Bindung |
| *Default dir* (`DCDEFAPP`/`DCDEFWIN`) | **weggelassen** | Verbraucher ist `set_default_path()` in `do_aopen` (`desksupp.c:896-912`); NewTOS' Startpfad setzt ueberhaupt kein Arbeitsverzeichnis |
| *Parameter* (`DCPMFULL`/`DCPMFILE`) | **weggelassen** | Verbraucher ist `build_selected_path` (`deskfun.c:903-931`) aus dem Ablegen-auf-Programm-Zweig von `fun_win2win` (`:958-970`); den hat NewTOS nicht |

Die Regel ist DESK02s: ein Schalter, der nichts schaltet, wird nicht gezeigt.
Beide fehlen **nicht** in diesem Dialog, sondern in **anderen** Funktionen —
darum eine Fundstelle statt eines toten Knopfs. Ihre `#E`-Byte-5-Bits
(`INF_E5_APPDIR`/`INF_E5_ISFULL`, `deskapp.c:110-111`) laufen unveraendert
durch, eine Datei, die sie traegt, ueberlebt.

**Abgrenzung zu DESK02: es gibt keine.** `ADSETPRE` (`inf_pref`) und
`ADDESKCF` (`inf_conf`) sind zwei Baeume hinter zwei Menuepunkten; kein Feld
kommt in beiden vor. `inf_pref` fuehrt die `G.g_c*pref`-Einstellungen,
`inf_conf` `G.g_appdir`/`G.g_fullpath`, `menu_shortcuts[]` und zwei Anzeigen.

## 2. Der groessere Teil lag nicht im Dialog

Der Kuerzel-Editor bearbeitet ein Feld, das es in NewTOS **nicht gab**, und
rendert es in Menuetexte, die dafuer **keinen Platz hatten**. Sechs Stuecke
aus `deskmain.c` mussten mit, alle in `kernel/src/desk/menu.rs`:

| EmuTOS | NewTOS |
|---|---|
| `menu_shortcuts[NUM_SHORTCUTS]` (`:119`) | `MENU.shortcuts` + `shortcut_key`/`set_shortcut_key`/`shortcuts`/`set_shortcuts` |
| `shortcut_mapping[]` (`:169-260`) | `SHORTCUT_MAPPING: [Option<Item>; 33]` |
| `default_shortcuts[]` (`:269-307`) | `DEFAULT_SHORTCUTS` |
| `copy_menu_items()` (`:1755-1813`) | `MENU.labels`, gefuellt in `build_menu` |
| `install_shortcuts()` (`:1825-1858`) | `install_shortcuts()`, aus `install_menu_bar` |
| `recover_shortcuts()` (`deskinf.c:1062-1090`) | `recover_shortcuts()` |
| `lookup_ascii_shortcut()` (`:1169-1198`) | feldgetrieben statt fest verdrahtet |
| `case 'K'` / `#K` (`deskapp.c:961-975`, `:1183-1193`) | `InfData::shortcuts` in `inf.rs` |

**Die sichtbarste Aenderung dieses Tickets ist nicht der Dialog**, sondern
dass die Klappmenues jetzt Kuerzel zeigen: `copy_menu_items` gibt jedem Menue
`1 + SHORTCUT_SIZE` Zeichen mehr (`:1781`), `install_shortcuts` schreibt
`'^'` + Buchstabe in die letzten zwei (`:1845`). EmuTOS tut das seit jeher
(`deskmain.c:2013` sae​t `default_shortcuts`, `:2023` setzt sie ein); NewTOS
zeigte **gar keine**. Gemessene Kastenbreiten: Desk 22, File 23, View 20,
Options 30 Zeichen. Die Trennstriche werden dabei erzeugt statt als feste
Zeichenkette gefuehrt — `adjust_menu` tut genau das (`:1623`), und
`copy_menu_items` laesst sie aus der Breitenmessung heraus (`:1773`), weshalb
sie die Messung auch nicht verfaelschen.

## 3. Der Dialogbaum

`ADDESKCF` Zeichenraster fuer Zeichenraster (`desk_rsc.c:1603-1772`), abzueglich
der zwei weggelassenen Reihen — alles ab *Function keys* rueckt vier
Zeichenzeilen hoch, der Kasten verliert dieselben vier: **312 x 288**,
`form_center` → ROOT (164, 105), gemeldetes Rechteck (161, 102, 318, 294).
Objektliste in `deskinf.md`. Drei Dinge, die man nicht raten kann:

* **Zwei Farbworte.** `4480` (`0x1180`, deckend) fuer die drei Felder, die auf
  der Flaeche sitzen; `4352` (`0x1100`, durchscheinend) fuer die zwei
  Rollfelder. `te_thickness` ebenso je Feld (−1 rahmt, 0 nicht).
* **`DCMNUTXT` traegt keine 3D-Flagge** (`NONE` im Baum), bleibt also ein
  weisser Kasten auf der grauen Flaeche.
* **`adjust_3d_positions`** (`deskmain.c:1647-1656`) schiebt `DCFUNPRV` und
  `DCMNUPRV` um `3 * ADJ3DSTD` = 6 px nach links, „to avoid button overlap".
  Woertlich uebernommen.

## 4. Benannte Abweichungen

1. **Zwei Radiogruppen fehlen** (§1). Fundstellen dort.
2. **Objektnummern sind NewTOS'.** Durch die zwei weggelassenen Gruppen
   verschieben sich die Indizes gegen `desk_rsc.h:191-211`; die **Namen** sind
   EmuTOS'. Genau wie `inf_pref` es in DESK02 macht.
3. **`install_shortcuts` haengt an `install_menu_bar`,** nicht an einer
   eigenen Startzeile. EmuTOS ruft es in `deskmain` nach `app_start`
   (`:2023`); bei uns ist `install_menu_bar` die Stelle, die nach
   `inf::load_and_apply` kommt, und der Aufruf ist idempotent, also kostet die
   Wiederholung aus dem Vollbild-Neuaufbau nichts. `do_read_inf` ruft es
   zusaetzlich auf — EmuTOS bekommt das geschenkt, weil `app_read_inf` den
   ganzen Desktop neu startet (`deskmain.c:920-926`), NewTOS wendet an Ort und
   Stelle an.
4. **`get_all_funkeys` laeuft ueber `desk::inf`s Sitzungsmodell,** nicht ueber
   `G.g_ahead`. EmuTOS fuehrt Desktop-Symbole und Zuordnungen in **einer**
   Liste; NewTOS trennt sie (seit Install-Application), und die Haelfte mit
   `a_funkey` ist die Zuordnungsliste.

## 5. Was gemessen wurde

QEMU `-M raspi4b`, 640x480, `-display none`, QMP-Screendumps, Kratzkopie des
IKBD-Injektors (`method/make_scratch_desk05.sh`). Sechs Laeufe, **0
`[EXC]`/`[PANIC]`** in allen sechs Protokollen.

* **Start** (`boot-desktop.png`) — Produktkern, unveraenderter Desktop.
* **`conf`** — `Options` zeigt *Desktop configuration...* schwarz und `^R`/`^S`
  an den beiden Punkten, die sie haben. Der Dialog misst ROOT
  (164, 105, 312, 288) = `form_center`s Wert. Schritte durch die Kuerzelliste:
  Open (`^O`) → Info/Rename... (`^I`) → Search... (kein Kuerzel). `Clear all`
  + `Cancel`: das File-Menue ist **pixelgleich** zu vorher (0 Pixel
  Unterschied) — `recover_shortcuts`. Der Desktop nach `Cancel` ist
  **pixelgleich** zum Ausgangsbild.
* **`edit`** — Backspace, `x`, `OK`: im File-Menue steht `Open ^X` statt
  `^O` (Ausschnittvergleich `crop-open-before-edit.png` /
  `crop-open-after-edit.png`), und `Save desktop` schreibt
  `#K 58 49 00 4E 48 55 57 41 00 44 00 5A ...` — erstes Byte `58` = `'X'`,
  Zeile zwischen `#Q` und `#W` wie `app_save` sie ordnet.
* **`cold`** — Kaltstart aus genau diesem Abbild: `menu_shortcuts` kommt als
  `[88, 73, 0, 78, ...]` zurueck, und das File-Menue ist **pixelgleich** zum
  Bild nach der Aenderung.
* **`okpl`** — `OK` ohne Aenderung: Desktop **pixelgleich** (0 Pixel), und das
  File-Menue pixelgleich zum Zustand vor dem Dialog (`install_shortcuts` ist
  idempotent).
* **`key`** — `Ctrl-S` schreibt die `NEWDESK.INF`: das umgebaute
  `lookup_ascii_shortcut` feuert.
* `fsck_msdos -n` auf den FAT16-Partitionen aller vier beschriebenen Abbilder:
  Phase 1/2/3 ohne Fehler.

## 6. Bau

`cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat` vor
**jedem** Board.

    cargo xtask build --board rpi4       tos-api 64 / kernel 94    (Basis 64/94)
    cargo xtask build --board qemu-virt  tos-api 64 / kernel 94    (Basis 64/94)
    cargo clippy --features board-rpi4        tos-api 80 / kernel 125 (Basis 80/125)
    cargo clippy --features board-qemu-virt   tos-api 80 / kernel 125 (Basis 80/125)
    grep -c "desk/" auf beiden Clippy-Laeufen:  0
    cargo run -p xtask -- test   932 passed / 0 failed / 1 ignored (Basis 932/0/1)

Protokolle in `measurement-2026-09-23-desk05/build/`.

**Kein neuer Hosttest.** Der Kuerzelpfad liegt vollstaendig in `kernel`
(`no_std`, aarch64) und ist vom Hostlauf nicht erreichbar; `tos-api` hat kein
Stueck davon. Der Beleg ist deshalb der Board-Lauf (§5), der die Kette
`#K` → `menu_shortcuts[]` → Menuetext → `Ctrl-S` an beiden Enden anfasst.

## 7. Offen / Folgetickets

* **Pi-Sichtabnahme** — HW-Punkt 39 in `docs/api/desktop/_hw-test-2026-09-17.md`.
  Nur dort zu pruefen: die 1920x1080-Optik und die Doppelte-Taste-Rueckfrage
  (`STDUPCUT`), die zwei Tastatureingaben hintereinander braucht.
* **`set_default_path` fehlt im Startpfad** (`desksupp.c:896-912`) — ein
  gestartetes Programm erbt kein Arbeitsverzeichnis. Blockiert *Default dir*.
* **Ablegen-auf-Programm fehlt** (`deskfun.c:958-970` +
  `build_selected_path`, `:903-931`). Blockiert *Parameter*.
* **`ins_app_dialog` hat kein Funktionstasten-Feld** (`deskins.c`, `APFUNKEY`)
  — darum bleibt der Anzeiger in diesem Dialog leer.
* **`Set background` steht im falschen Menue.** `BACKGRND` ist Objekt 46 und
  damit das **letzte Element des View-Menues** (`desk_rsc.h:35-46`); NewTOS
  fuehrt es als erstes Element von *Options*. Nicht angefasst.
