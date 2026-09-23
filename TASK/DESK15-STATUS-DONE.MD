# DESK15 — STATUS: DONE (2026-09-23)

**Auftrag:** Erst den Leser bauen — `win_bldview` muss das Symbol eines
Fensterobjekts aus der `ANODE`-Liste beziehen koennen, mit der Endungsregel
als Rueckfall —, dann `ADINSWIN` und den `STICNTYP`-Zweig in `ins_icon`. Wenn
die beiden Haelften trennbar sind: die Fenstersymbol-Haelfte bauen und die
andere klar als Folgeticket melden.

Basis: Branch `desk-rebuild`, HEAD `f206a2a`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung: [`measurement-2026-09-23-desk15/`](../measurement-2026-09-23-desk15/).
Spezifikation: `docs/api/desktop/deskins.md`
(§ `install_window_icon` / `ins_icon`s `STICNTYP`-Zweig),
`docs/api/desktop/deskwin.md` (§ `win_icalc`).

---

## 1. Die Analyse des Tickets — gegen die Quelle geprueft

**Richtig:** die Reihenfolge. Ohne den Leser waere `ADINSWIN` ein Bedienelement
ohne Wirkung, und `ins_icon(0)` ging tatsaechlich auf den `#else`-Zweig
(`references/emutos/desk/deskins.c:1262-1263`).

**Richtig:** die Gatter. Beide Schalter stehen im Profil Falcon030/TOS4
(`make 512`) auf **1** — `CONF_WITH_WINDOW_ICONS`
(`include/config.h:1866-1868`), `CONF_WITH_DESKTOP_SHORTCUTS` (`:1782-1784`).
Die `0`-Vorgaben bei `:322-327` stehen im Block `#ifdef TARGET_192`
(`:210-336`), und `make 512` definiert `TARGET_512` (`Makefile:560-562`), das
in `config.h` gar nicht vorkommt. Keine der beiden ist „im Referenzbau aus".

**Zu korrigieren:** „dieselbe fehlende Ebene". Es sind zwei Listen.

* Ein **Fenstersymbol** ist ein `AF_WINDOW`-Knoten: `#I`/`#N` fuer eine Datei,
  `#D` fuer einen Ordner, `a_pdata` = der blosse Dateiname, `a_pappl` leer
  (`deskins.c:1028-1052`, `deskapp.c:325-333`, `:1223-1244`). In NewTOS gehoert
  er damit in die **Zuordnungs**haelfte — `desk::inf`s Session-Modell —, nicht
  in `DeskState::g_ahead`.
* Eine **Desktop-Verknuepfung** ist ein `AF_ISDESK`-Knoten vom Typ
  `AT_ISFILE`/`AT_ISFOLD`: `#X`/`#V`, mit Gitterplatz und Laufwerksbuchstaben
  (`deskapp.c:333-343`). Der gehoert in `g_ahead`, und genau dort fehlt die
  Variante.

Die Haelften sind also trennbar. Gebaut ist die erste; die zweite ist
[DESK30](DESK30.md).

---

## 2. Was gebaut wurde

### 2.1 Der Leser — `win_icalc`

`kernel/src/desk/win.rs`:

* `DeskState::win_icalc(slot, pidx)` — EmuTOS `deskwin.c:377-384`. Loest den
  Eintrag nach `AT_ISFOLD`/`AT_ISFILE` und Namen gegen die `ANODE`-Liste auf,
  mit dem `p_spec` des Fensters fuer den voll qualifizierten
  `a_pappl`-Vergleich.
* `build_item` ruft ihn **vor** der Ansichtsverzweigung, wie `deskwin.c:455`
  und `:462` es tun, und die Symbolansicht nimmt
  `i_index = f_isap ? a_aicon : a_dicon` (`:465-467`), durch
  `icons::icon_kind` gereicht.
* Rueckfall, wenn kein Knoten den Namen beansprucht (oder die Nummer kein
  Bild hat — `IG_REMOV`, DESK16): die bisherige Endungsregel
  `ftype::classify`.

Genannte Abweichung: NewTOS legt **kein** `f_pa`/`f_isap` im `FNODE` ab.
EmuTOS tut das fuer seinen zweiten Verbraucher, `do_aopen`s Start eines
installierten Programms (`desksupp.c:857-875`); NewTOS' `do_aopen` loest die
Zuordnung selbst ueber `inf::lookup_association` auf, also gibt `win_icalc`
die Antwort zurueck statt sie zu speichern, und `install_window_icon` stellt
dieselbe Anfrage noch einmal.

### 2.2 Die Listenprimitive

`kernel/src/desk/inf.rs`, neben dem Speicher, den sie bearbeiten:

* `app_afind_by_name(atype, pspec, pname) -> Option<Afind>` —
  `deskapp.c:1437-1466`, Zeile fuer Zeile: `ignore`-Knoten ueberspringen,
  `a_type == atype` fordern, erst `wildcmp(a_pdata, pname)` (`*pisapp = FALSE`),
  dann `a_pappl` — `wildcmp`, wenn es mit `*`/`?` beginnt, sonst `strcmp`
  gegen Verzeichnis(`pspec`) + `pname`. Die `ignore`-Maske ist allein
  `AF_ISDESK`: `AF_VIEWER` ist `CONF_WITH_VIEWER_SUPPORT`, das dieser Desktop
  nicht hat, also kann es kein Knoten tragen.
* `allocate_window_anode(atype)` — `deskins.c:1028-1052`, haengt **vorn** an
  (`app_alloc` verkettet am Kopf, `deskapp.c:204-219`), damit ein Name vor den
  `*.*`-Faengern gefunden wird.
* `set_anode_data(idx, name)` — `scan_str(new_name, &pa->a_pdata)` (`:1175`).
* `set_icon(idx, icon)` — `deskins.c:1012-1022`. Setzt `a_dicon` immer und
  `a_aicon` zusaetzlich, wenn der Datenname ausfuehrbar ist. EmuTOS haelt das
  in `AF_ISEXEC` und laesst `app_save` daraus `#I`/`#N` machen
  (`deskapp.c:1230-1231`); `InfAnode` hat kein Flagwort, also **ist** die
  Kennung das Flag und wird hier gesetzt.
* `app_free(idx)` — `deskapp.c:227-247`.

Genannte Abweichung: EmuTOS' `app_alloc` kann scheitern, wenn der feste
`NUM_ANODES`-Vorrat (64) voll ist, und `install_window_icon` antwortet dann
`change = -1` (`deskins.c:1170-1174`). NewTOS' Liste ist ein `Vec` ohne festen
Vorrat — eine ROM-Knappheit, deren Fehlen nur als „die Warnung kommt nie"
sichtbar waere —, also hat dieser Arm keine Entsprechung.

### 2.3 `desk_inf_data2` — Reihenfolge und drei eigene Zeilen

Mit dem Leser wird die **Suchreihenfolge** der generischen Typknoten
beobachtbar. `desk_inf_data2` listet die drei `*.*`-Faenger zuerst
(`deskapp.c:181-189`), aber `app_alloc` haengt jeden gelesenen Knoten vorn an
(`:204-219`) — `G.g_ahead` laeuft also andersherum, und `app_save` stellt die
Dateireihenfolge mit dem `app_revit`-Paar um den Schreibvorgang wieder her
(`:1213`, `:1285`). NewTOS hat keine kopfverkettete Liste zum Umdrehen:
`InfData::anodes` **ist** die Liste und wird in dieser Reihenfolge
geschrieben. `default_type_anodes()` steht deshalb jetzt in der **Such**-
reihenfolge; die Zeilenfolge auf der Karte ist damit die Umkehrung von
EmuTOS'. Genannte Abweichung — das Ergebnis der Aufloesung ist identisch, und
ohne sie faengt `#F FF 07 @ *.*@` jedes `FOO.PRG` ab.

Drei Zeilen sind NewTOS' eigene, auf derselben Grundlage wie ADR-018s
Dateinamen:

    #G 06 FF *.NAP@ @      native GEM-Anwendung
    #F 06 FF *.NCP@ @      natives Konsolenprogramm
    #P 06 FF *.NTP@ @      natives Programm mit Kommandoschwanz

`desk_inf_data2` ist genau die Stelle, an der EmuTOS erklaert, welche Endungen
ein *Programm* benennen. Die klassischen fuenf bleiben (sie sind weiter die
Namen der m68k-Familie). `is_executable` zieht nach: EmuTOS'
`exec_ext[] = {"TOS","TTP","PRG","APP","GTP"}` (`deskins.c:984`) ist NewTOS'
`FileType::M68k`, also lautet die Frage „kein `FileType::Doc`".

`build_inf()` legt das synthetisierte Modell zusaetzlich in die Session.
EmuTOS **parst** seinen `infbuf` in das lebende `G.g_ahead`
(`deskapp.c:745-752`, `:838-841`), damit `app_afind_by_name` immer die acht
generischen Knoten hat und `install_window_icon`s `if (!pa) … "can't happen"`
(`deskins.c:1157-1161`) wirklich nicht passieren kann. Bisher blieb das Modell
leer, und auf jeder frischen Karte passierte es.

Bekannte Folge, wie bei EmuTOS: eine `NEWDESK.INF`, die eine aeltere NewTOS
geschrieben hat, traegt nur die acht EmuTOS-Zeilen — dort bekommt ein
`.NAP`/`.NCP`/`.NTP` das Dokumentbild, bis die Datei einmal neu gespeichert
wird. EmuTOS verhaelt sich bei einer `EMUDESK.INF` aelter als eine eigene
`desk_inf_data2`-Aenderung genauso und tut nichts dagegen.

### 2.4 Der Kasten und `ins_icon`

`kernel/src/desk/ins.rs`:

* `is_executable` (`deskins.c:988-1004`).
* `install_window_icon(pf)` (`:1061-1222`) — `ADINSWIN` mit den Objektnummern
  aus `desk_rsc.h:225-237` und der Zeilenfolge aus `desk_rsc.c:2038-2135`,
  auf dem Stapel gebaut und durch `start_dialog`/`form_do`/`end_dialog`
  gefahren. Mit `FNODE`: Name fest und nicht `EDITABLE`, `edit_start = 0`,
  beide Typknoepfe vorgewaehlt **und** `DISABLED` (`:1093-1104`). Ohne:
  Name `EDITABLE` und leer, `edit_start = IW_NAME`, *File* vorgewaehlt, beide
  aktiv (`:1105-1112`). `curr_icon = pa ? pa->a_dicon : 0`, begrenzt
  (`:1115-1120`), ueber `offered_icons` geschritten.
  `IW_INST`/`IW_REMV` loesen den Knoten auf, vergleichen `a_pdata` mit dem
  (ggf. bearbeiteten) Namen (`identical`, `:1164`) und handeln danach
  (`:1166-1192`); `STNOMTCH` geht woertlich an `form_alert`, wie `STDRIVID` es
  schon tat. Die Schleife bricht **vor** dem Loeschen von `SELECTED` ab und
  waehlt nur fuer `IW_UP`/`IW_DOWN` ab (`:1196-1201`) — umgekehrt zu
  `install_desktop_icon` (`:943-945`), und so nachgebaut.
* `ins_icon(sobj)` (`:1234-1311`) hat jetzt drei Zweige: `STICNTYP` bei
  `sobj == 0`, die Desktop-Schleife, und die Fensterauswahl mit **umgekehrtem**
  Vorzeichen (`rebuild--`, `:1298`).

`kernel/src/desk/main.rs`: `IICNITEM` bekommt den `else if (rebld < 0)`-Arm
`win_bdall(); win_shwall();` (`deskmain.c:868-875`).

`kernel/src/desk/dir.rs`: `fmt_str`/`unfmt_str` sind `pub(super)` — in EmuTOS
liegen sie in `util/optimize.c` und werden von beiden Stellen benutzt. Sonst
unveraendert.

Genannte Abweichungen: (1) die `ADJ3DSTD`-Verschiebung von `IW_DOWN`
(`:1117-1123`, `:1210-1216`) hat keine Entsprechung — der Baum wird bei jedem
Aufruf frisch mit den gewachsenen Rechtecken gebaut, genau wie `ADINSDSK`.
(2) EmuTOS laeuft die lebende `p_flist` ueber die Dialoge hinweg ab; NewTOS
schnappt die ausgewaehlten Eintraege vorher weg, weil der Dialog AES-Traps
absetzt und nicht unter der Desktop-Sperre laufen kann. Der Dialog fasst die
Dateiliste nicht an, beide Laeufe sehen dieselben Eintraege.

---

## 3. Belege

`measurement-2026-09-23-desk15/` — QEMU raspi4b, 640x480, `-display none`,
QMP-Screendump, scratch-Kopie mit dem H02-IKBD-Injektor. **Jeder** Klickpunkt
stammt aus einem seriellen Dump (`method/desk15_patch.py`) bzw. aus einer
Messung am Screendump (`method/alertbox.txt`); keiner ist geraten.

* `screendumps/p-win.png` + `icons-cmp.png` — der Leser: `.NCP`/`.NAP` tragen
  das Programmbild, `.TXT` das Dokumentbild, ueber die `ANODE`-Liste.
* `q-dialog.png` — `ADINSWIN` mit `FNODE`; serial: `pa=Some((9,false,-1,7))`
  (der generische `#N FF 07 @ *.*@`), `curr_icon=5`.
* `a-f-stepped.png` / `a-g-installed.png` / `a-i-saved.png` — drei Schritte auf
  den Papierkorb, `Install`, und **nur** `README.TXT` aendert sich; dann
  `Save desktop...`. Die geschriebene Datei:
  `method/a-NEWDESK.INF.after-save`, Zeile `#N FF 03 @ README.TXT@`.
* `b-a-cold.png` — **Kaltstart**: `README.TXT` traegt den Papierkorb wieder.
* `c1-c-alert.png` — `Please select icon type.` mit
  `Desktop | Window | Cancel`.
* `c2-d-dialog.png` — die *Window*-Antwort: leeres, beschreibbares Namensfeld,
  beide Typknoepfe aktiv; serial: `pa=None curr_icon=0`.
* `d-d-dialog.png` / `d-e-removed.png` + `remove-cmp.png` — `Remove` nimmt die
  Zuordnung zurueck, die Datei faellt auf das Dokumentbild.

`[EXC]`/`[PANIC]`: 0 in allen sieben raspi4b-Logs und im qemu-virt-Bootlog.
`fsck_msdos -n` auf der FAT16-Partition jeder geschriebenen Karte: Phase 1/2/3,
keine Fehler (`build/fsck.txt`). Kein `*.img` im Messverzeichnis.

### Bau

    cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
    cargo xtask build --board rpi4        tos-api 64 / kernel 94   (Basis 64/94)
    cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
    cargo xtask build --board qemu-virt   tos-api 64 / kernel 94   (Basis 64/94)

    cargo clippy --features board-rpi4       tos-api 80 / kernel 125 (Basis 80/125)
    cargo clippy --features board-qemu-virt  tos-api 80 / kernel 125 (Basis 80/125)
    Treffer in kernel/src/desk/: 0

    cargo run -p xtask -- test   938 bestanden / 0 fehlgeschlagen / 1 ignoriert

---

## 4. Offen

* [DESK30](DESK30.md) — `CONF_WITH_DESKTOP_SHORTCUTS`, die zweite Haelfte der
  urspruenglichen Meldung. Der DESK18-Nebenbefund (*Default dir* wird erst mit
  Verknuepfungen und `fun_win2win`s Ablegen-auf-Programm sichtbar) haengt dort.
* Pi-4-Punkt **44** in `docs/api/desktop/_hw-test-2026-09-17.md`: die Optik in
  1920x1080, die echte Maus, die **Tastatur** im Namensfeld (den
  `pf == None`-Weg kann QEMU raspi4b nicht tippen) und eine Zuordnung ueber
  mehrere Laufwerke.

## 5. Nebenbefunde (nicht angefasst)

* `desk::ins::selftest()` und `desk::inf::selftest()`/`selftest_install()`
  haengen in keinem Bootpfad — sie sind aus dem Baum heraus nicht aufrufbar
  (bekannt, vgl. MEMORY „Unverdrahtete Selftests verrotten still").
