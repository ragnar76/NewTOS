# DESK30 — Desktop-Verknuepfungen (`CONF_WITH_DESKTOP_SHORTCUTS`)

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke — gemeldet in DESK15

Abhaengigkeiten: [DESK15](DESK15.md) (erledigt)

## Sachverhalt

DESK15 hat die Fenstersymbole gebaut (`CONF_WITH_WINDOW_ICONS`), also den
Leser `win_icalc` und den `ADINSWIN`-Kasten. Die **zweite** Haelfte derselben
Meldung ist offen: `CONF_WITH_DESKTOP_SHORTCUTS` — eine Datei oder ein Ordner
als Symbol **auf dem Desktop**.

Sie ist nicht dieselbe Ebene. Ein Fenstersymbol ist ein `#I`/`#N`/`#D`-Knoten
der Zuordnungsliste (`desk::inf`s Session-Modell), eine Verknuepfung ist ein
`#X`/`#V`-Knoten der **Desktop**-Liste `DeskState::g_ahead`, und die hat bis
heute kein `AT_ISFILE`/`AT_ISFOLD` (`global::Anode` traegt weder `a_pdata`
noch `a_pargs`, siehe ihr Doc-Kommentar).

`CONF_WITH_DESKTOP_SHORTCUTS` ist im Profil Falcon030/TOS4 (`make 512`) **1**
(`references/emutos/include/config.h:1782-1784`; die `0` bei `:325-327` steht
in `#ifdef TARGET_192`), also eine echte Luecke.

## Arbeit

Quellen: `references/emutos/desk/deskins.c` — `ins_shortcut` (`:1341-…`),
`is_installed` (`:71-116`), die `AT_ISFILE`/`AT_ISFOLD`-Zweige von
`install_desktop_icon` (`:781-978`), `ins_app`s Verknuepfungszweig
(`:619-720`); `references/emutos/desk/deskapp.c` — `app_parse`/`app_save` fuer
`#X`/`#V` (`:333-344`, `:1223-1244`) und die Vertauschung von
`a_pappl`/`a_pdata` unterhalb von `inf_rev_level 2` (`:404-415`);
`references/emutos/desk/deskfun.c` — `fun_win2win`s Ablegen-auf-Programm.

Reihenfolge wie bei DESK15: zuerst die Datenebene (`Anode` bekommt
`a_type = File/Folder`, `a_pdata`, `a_flags`-Ersatz), dann `app_blddesk` und
`app_afind_by_id`s Konsumenten (Doppelklick oeffnet die Datei, Ablegen darauf
startet sie), dann `ins_shortcut` und die Dialogzweige.

Nebenbefund aus DESK18, der hier haengt: EmuTOS unterscheidet die beiden
Stellungen von *Default dir* nur ueber Verknuepfungen und `fun_win2win`s
Ablegen-auf-Programm — erst diese Haelfte macht den Schalter sichtbar.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
