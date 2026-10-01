# DESK34 — STATUS: DONE (2026-09-24) — implementiert, Pi-Abnahme offen

**Auftrag:** `do_fopen`s Fallunterscheidung nachziehen, sodass ein Ordner im
Fenster navigiert statt ein neues Fenster zu oeffnen; der Laufwerkspfad muss
weiterhin ein neues Fenster bekommen.

Basis: Branch `desk-rebuild`, HEAD `a84b0ba`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`.

Messung:
[`measurements/measurement-2026-09-24-desk34-desk35/`](../measurements/measurement-2026-09-24-desk34-desk35/)
§2. Spezifikation: `docs/api/desktop/deskmain.md`, Abschnitt *opening a folder
navigates the window (DESK34)*. Hardwarepunkt 59 in
`docs/api/desktop/_hw-test-2026-09-17.md`.

---

## 1. Die Quelle

`do_fopen` (`references/emutos/desk/desksupp.c:1089-1160`), gekuerzt auf die
Entscheidung:

```c
#if CONF_WITH_DESKTOP_SHORTCUTS
    if (pw->w_flags & WN_DESKTOP) { … new_win = TRUE; }   /* :1112-1127 */
    else
#endif
    if (allow_new_win) {
        graf_mkstate(&junk, &junk, &junk, &keystate);
        if ((keystate & MODE_SCA) == MODE_ALT)            /* :1130-1135 */
            new_win = TRUE;
    }
    if (new_win) { pw = win_alloc(curr); rc_copy(…, &t); }  /* :1141-1149 */
    else         { pn_close(&pw->w_pnode); }               /* :1151-1154 */
    do_diropen(pw, new_win, curr, app_path, &t, TRUE);     /* :1156 */
```

Aufrufer, alle drei:

| Ort | `allow_new_win` |
|---|---|
| `do_open` (`desksupp.c:1259`) — Doppelklick *und* *File → Open* | `TRUE` |
| `w_setpath` (`deskfun.c:824`) — *Close folder* / `WM_CLOSED` | `FALSE` |
| `refresh_window` (`desksupp.c:1692`) | `FALSE` |

Und `MODE_SCA`/`MODE_ALT` sind `include/biosdefs.h:90-94`:
`MODE_SCA = RSHIFT|LSHIFT|CTRL|ALT`, `MODE_ALT = 0x08` — die Gleichheitsprobe
verlangt **allein** Alt.

## 2. Welcher unserer Aufrufer heute welchen Wert setzt

Vor der Aenderung am Quelltext festgestellt:

* `main::open_item`s `OpenTarget::Folder`-Arm rief unbedingt
  `open_dir_window` — das ist `do_diropen(pw, TRUE, …)`, also dauerhaft
  `new_win = TRUE`. **Das ist der Defekt.** Beide Wege dorthin
  (`hndl_button`s Doppelklick und `do_open_selected` = *File → Open*)
  entsprechen EmuTOS' `do_open`, also `allow_new_win = TRUE`.
* `main::open_root_window` ist `do_dopen` (`desksupp.c:1041-1069`) — ein
  **eigener** Aufrufer, der `win_alloc` unbedingt ruft. Er geht gar nicht
  durch `do_fopen` und blieb unveraendert.
* `main::navigate_window` war bereits `do_diropen(pw, FALSE, …)` in allem
  ausser dem Namen, benutzt von *Close folder*.

Der dritte EmuTOS-Fall, `WN_DESKTOP`, ist hier nicht erreichbar: NewTOS hat
keinen `AT_ISFOLD`-Desktop-`ANODE` (`kernel/src/desk/ins.rs` Modulkopf,
DESK30). Wenn der kommt, wird er der zweite `new_win = TRUE`-Erzeuger in
`do_fopen`.

## 3. Was gebaut wurde

`kernel/src/desk/main.rs`:

* neue Funktion `do_fopen(handle, spec, title, allow_new_win)` — die
  Fallunterscheidung, mit `graf_mkstate` im Moment des Oeffnens (nicht der
  Tastenzustand, den der Klick trug);
* `MODE_RSHIFT`/`MODE_LSHIFT`/`MODE_CTRL`/`MODE_ALT`/`MODE_SCA` als benannte
  Konstanten (`biosdefs.h:90-94`);
* `open_item`s `Folder`-Arm ruft `do_fopen(handle, …, true)`;
* `navigate_window` ist jetzt als `do_diropen(pw, FALSE, …)` dokumentiert und
  vollstaendig: `desk_verify` (`desksupp.c:459`) kam dazu, und `w_cvcol = 0`
  fiel weg, weil EmuTOS nur `w_cvrow` zuruecksetzt (`:1258`,
  `deskfun.c:823`).

Kein Stub, keine neue Abhaengigkeit.

## 4. Benannte Abweichungen

1. `graf_growbox`-Zoom und `act_chg`-Abwahl des Quellsymbols (`do_wopen`,
   `desksupp.c:294-322`) fehlen — auf **jedem** NewTOS-Fensteroeffnungsweg,
   also eine vorbestehende Luecke, nicht eine neue. Die Abwahl ist im
   `!new_win`-Zweig unbeobachtbar: `win_bldview` baut den Elementbaum aus dem
   neuen Verzeichnis neu, das `OBJECT` mit `OB_SELECTED` gibt es dann nicht
   mehr.
2. `wind_update(BEG/END_UPDATE)` klammert bei EmuTOS `do_diropen` nicht, also
   auch hier nicht — nur erwaehnt, weil DESK35 die Klammer um `cnx_get`
   ebenfalls auslaesst.

## 5. Gemessen

QEMU `-M raspi4b`, 640×480, `-display none`, QMP-Screendumps, Karte aus
`tools/make-sd-image.sh` (+ `method/seed.sh`). Jede Klickkoordinate aus dem
`d-probe2`-Seriendump.

| | HEAD (`d34-before-*`) | jetzt (`d34-after-*`) |
|---|---|---|
| `SUB` im `C:\`-Fenster geoeffnet | **zweites Fenster** (Slot 1 / Handle 2, Titel `C:\SUB\`), `C:\` steht dahinter | Slot 0 / Handle 1 **bleibt**, `p_spec` `C:\*.*` → `C:\SUB\*.*`, Titel `C:\SUB\` |
| danach das Laufwerkssymbol | neues Fenster (Slot 2) | neues Fenster (Slot 1 / Handle 2) |

0 `[EXC]`/`[PANIC]` in beiden Laeufen; `fsck_msdos -n` sauber auf beiden
Kartenabbildern (`build/fsck.txt`).

## 6. Was offen bleibt

* **Alt+Doppelklick** — braucht eine echte Tastatur; der IKBD-Injektor setzt
  keinen Modifizierer-Zustand, den `graf_mkstate` liest. Pruefpunkt 59.
* **Reale Pi-Gegenprobe** insgesamt: dort wurde der Defekt gefunden.
* `WN_DESKTOP` bleibt unerreichbar bis DESK30.
