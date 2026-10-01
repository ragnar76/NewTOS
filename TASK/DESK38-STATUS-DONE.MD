# DESK38 — STATUS: DONE (2026-09-25) — implementiert, Pi-Abnahme offen

**Auftrag:** `do_info` (`desksupp.c:1288-1354`) nachziehen, Fall fuer Fall —
*Show info* gab es auf der Schreibtischflaeche ueberhaupt nicht, weil
`main::do_info_rename` fuer `G.g_cwin == DESKWH` sofort umkehrte.

Basis: Branch `desk-rebuild`, HEAD `12fddc1`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset` auf dem gemeinsamen Baum.

Messung:
[`measurements/measurement-2026-09-25-desk-rest/`](../measurements/measurement-2026-09-25-desk-rest/) §3.
Spezifikation:
[`docs/api/desktop/desksupp-redraw.md`](../docs/api/desktop/desksupp-redraw.md)
und [`deskinf.md`](../docs/api/desktop/deskinf.md). Hardwarepunkt **69**.

---

## 1. `do_info(curr)` — der Schalter

`i_find(G.g_cwin, curr, &pf, NULL)` liefert den regierenden `ANODE`; mehr als
sein `a_type` liest der Schalter nicht. Kein `ANODE` → `1` (weitermachen).

| Zweig | Was |
|---|---|
| `AT_ISFOLD`/`AT_ISFILE` | `inf_file_folder`, mit zwei Quellen (unten) |
| `AT_ISDISK` | `inf_disk(LOBYTE(ib_char))` — der Laufwerksbuchstabe aus dem Symbol |
| `AT_ISTRSH` | `trash_alert(pa)` — `STTRINFO` mit dem **aktuellen** Namen |
| `AT_ISPRNT` | `printer_alert(pa)` — `STPRINFO`, ebenso |

Rueckgabe wie EmuTOS: `1` weiter, `0` Abbruch, `-1` weiter und ein Fenster ist
markiert.

## 2. Die zwei Quellen des Datei-/Ordnerzweigs

Ein **Fenstereintrag** nimmt seine Felder aus dem `FNODE`, den das Objekt
traegt, und sein Verzeichnis aus `p_spec`. Eine **Verknuepfung** hat keinen
`FNODE`: EmuTOS sondiert die Datei, auf die sie zeigt
(`file_exists(pa->a_pappl, NULL)`), und baut sich einen `FNODE` auf dem Stapel
aus der DTA (`memcpy(&pf->f_attr, &dta->d_attrib, 23)`, `:1326`). Zeigt sie
ins Leere, kommt `remove_locate_shortcut` statt eines Kastens — genau wie beim
Oeffnen.

`win_find(DESKWH)` ist das Schreibtisch-Pseudofenster und nie NULL
(`deskwin.c:202-203`), also greift die `if (pw)`-Huelle auf dem Schreibtisch
immer. Auf dem Schreibtisch ist ein `AT_ISFILE`/`AT_ISFOLD`-Knoten
zwangslaeufig eine Verknuepfung — jeder Knoten von `G.g_ahead` ist
`AF_ISDESK`.

## 3. Die `SHOWITEM`-Schleife

`do_filemenu`s `SHOWITEM` (`deskmain.c:674-689`) ist eine Schleife ueber die
Auswahl; wir zeigten immer nur einen Kasten. Drei markierte Symbole geben jetzt
drei Kaesten nacheinander, und *Cancel* auf einem beendet die Reihe. Danach
`fun_rebld_marked()` (DESK31 Punkt 4) — **hinter** der Schleife, weil ein
Neuaufbau mittendrin die `FNODE`s wegwuerfe, die sie abschreitet. Ist
**nichts** markiert, bekommt das oberste Fenster den Laufwerkskasten, „just
like TOS" (`:683-684`).

## 4. `inf_disk` — der `ADDISKIN`-Kasten

Neu, samt allem, was darunter fehlte:

* `dir::count_ffs` (`deskinf.c:476-481`) — `d_doop(0, OP_COUNT, path, path)`;
  `None` ist EmuTOS' `!more`.
* `dosbind::dos_space` — `Dfree` mit EmuTOS' `buf[3]*buf[2]`-Rechnung, hier in
  64 Bit: eine 500-MB-FAT16-Partition laeuft EmuTOS' vorzeichenbehaftetem
  32-Bit-Bytezaehler ueber.
* `dosbind::dos_label` — `Fsfirst("X:\\*.*", 0x08)`; eine Suchmaske von genau
  `FA_VOL` trifft ausschliesslich Datentraegernamen.
* `main::valid_drive` (`desksupp.c:1810-1826`) — mit `STNODRIV`, denn der
  Alert gehoert bei EmuTOS **in** die Funktion.

Drei Eigenheiten der Quelle, die die Umsetzung behaelt: `total` wird gelesen
und nie gezeigt (das *used* kommt aus dem Lauf, nicht aus `total - avail`);
`inf_fifosz` zieht eins vom Ordnerzaehler ab (`:448` — der Laufwerkswurzel ist
kein Ordner, den jemand angelegt hat); und ein fehlendes Laufwerk sagt das
laut statt still.

## 5. `inf_file_folder` gibt drei Antworten

`Option<String>` machte *Cancel* und „OK, nichts geaendert" ununterscheidbar.
`do_info`s `0` braucht sie getrennt, also `InfFF::{Cancel, Continue,
Renamed(name)}` — EmuTOS' `0`/`1`/`-1` (`deskinf.c:487-491`).

**Benannte Abweichungen, beide aelter als DESK38 und hier nicht angefasst:**
EmuTOS benennt in `inf_file_folder` selbst um, in einer `while(1)`, die nach
einem `STRENAME`-*Retry* den ganzen Dialog neu zeigt (`:551,629-634`), und es
schreibt ein geaendertes Schreibschutz-Attribut zurueck (`:591-609`). Bei uns
benennt der Aufrufer einmal um, mit NewTOS' eigenem Fehlalert, und die
Attributzeile ist nur Anzeige. Der beobachtbare Unterschied ist das fehlende
*Retry*.

---

## Geaenderte Dateien

* `kernel/src/desk/main.rs` — `do_showitem`, `do_info`, `do_info_file_folder`,
  `show_item_info`, `trash_alert`, `printer_alert`, `anode_label`,
  `valid_drive` (alle neu); `do_info_rename` entfallen; `Item::InfoRename`
  zeigt auf `do_showitem`
* `kernel/src/desk/inf.rs` — `inf_disk`, `InfFF` (neu); `inf_file_folder`
  (Rueckgabetyp)
* `kernel/src/desk/dir.rs` — `count_ffs`, `file_exists`, `FileInfo` (neu)
* `kernel/src/desk/dosbind.rs` — `dos_space`, `dos_label`, `DFREE`, `FA_VOL`
* `docs/api/desktop/desksupp-redraw.md`, `deskinf.md`, `deskdir.md`,
  `docs/api/desktop/_hw-test-2026-09-17.md` (Punkt 69)

## Geprueft

* `cargo run -p xtask -- test` — **944 passed / 0 failed / 1 ignored**.
* Frischer Build je Board: **tos-api 64 / kernel 94**, Delta 0.
* `clippy` beide Boards: **tos-api 80 / kernel 125**, Delta 0, null Treffer in
  `kernel/src/desk/`.
* QEMU raspi4b, Laeufe `d38a` und `d38b`:

  ```
  [d38] do_info(curr=9)  cwin=0 atype=Disk      DISK INFORMATION, NEWTOS,
                                                11 Ordner / 40 Dateien,
                                                1597248 belegt, 65241088 frei
  [d38] do_info(curr=10) cwin=0 atype=Trash     "TRASH is used to ..."
  [d38] do_info(curr=11) cwin=0 atype=Printer   "PRINTER is a printer."
  [d38] do_info(curr=37) cwin=1 atype=File      Fenstereintrag README.TXT
  [d38] do_info(curr=43) cwin=0 atype=File      Verknuepfung README.TXT
  ```

  `cwin=0` ist DESKWH — der Fall, der vorher sofort umkehrte. Je ein
  Screendump. Null `[EXC]`, null `[PANIC]`, `fsck_msdos -n` sauber.

## Offen

* **Hardwareabnahme**, Punkt 69. Dort zaehlt `count_ffs` ueber vier echte
  Partitionen statt ueber ein 64-MiB-Abbild — wie lange das dauert und ob die
  Sanduhr dabei steht, ist der eigentliche Punkt der Hardwareprobe.
* Die Waise einer Verknuepfung (`remove_locate_shortcut` aus `do_info` heraus)
  ist gebaut, aber nicht gefahren; sie braucht eine zwischen zwei Sitzungen
  verschobene Zieldatei und haengt an Punkt 63.5.
