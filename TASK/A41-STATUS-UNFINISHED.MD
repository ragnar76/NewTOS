# A41 — Quittung

**Status: UNFINISHED** · 2026-09-19 · **Spec fertig, Implementierung offen.**
Nur Dokumentation nach Track A (`docs/SOURCE-ANALYSIS-PROTOCOL.md`); keine
`.rs`-/`.py`-Datei angefasst, kein Build, kein Test gefahren, nichts
committet. `TASK/A41.md`s Kopfzeile (Status **OFFEN**) bleibt unveraendert —
der zugrunde liegende Defekt (`mkrsc.py` weist CICON-Resourcen weiterhin ab)
ist nicht behoben.

## Quelle

`references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df` (gepinnt,
Befund B11 verlangt), Zielprofil Falcon030/TOS4, `make 512`.
`CONF_WITH_COLOUR_ICONS` ist dort **1** (`include/config.h:1266-1267`).

## Was extrahiert wurde

`docs/api/aes/resource-fileselector.md` — neuer Abschnitt **§5 „Colour icons
(CICON)"** (§5.1–§5.11, ca. 460 Zeilen): Neuformat-Erkennung + Erweiterungsfeld
(`get_ciconblkptr`), CICONBLK-Zeigertabelle, das Dateiformat von CICONBLK und
CICON vor dem Fixup (Byte fuer Byte hergeleitet aus `fixup_all_ciconblks`/
`fixup_colour_icons`, `gemrslib.c:185-256`), `fix_objects`' `G_CICON`-Index→
Zeiger-Umwandlung, der volle Ladepfad `transform_all_cicons` (Fehlerpfade bei
`dos_alloc_anyram`-Fehlschlag eingeschlossen), die Auswahlregel `best_match`
woertlich mit Zeile, die Freigabe `free_cicon_buffers`/`rs_free`, ein
NRSC-Formatvorschlag (v11/v12) fuer den Konverter/Lader, eine Bauliste mit
Abnahmen fuer den Developer, und vier explizit dem Anwender vorgelegte offene
Entscheidungen (§5.11).

`docs/api/aes/object-form.md` — Ergaenzung nach Punkt 6 des `just_draw`-
Spaziergangs: der volle `G_CICON`-Zeichenarm (`gr_gicon`, Daten/Masken-
Auswahl, `gr_colourblit`, die `dither_mask`-Verdunklung ohne `sel_data`,
Zeichen/Label wie bei G_ICON), plus ein Eintrag in „NewTOS implementation
notes" und die Konfigurationsnotiz.

Jede Aussage traegt `datei:zeile`; wo aus zwei Quellenzitaten geschlossen statt
woertlich zitiert wurde, ist es als **Inferenz** gekennzeichnet (z. B. die
Koordinaten-Gegenprobe in §5.5, die keine Annahme ueber die Zellgroesse
braucht, weil das Low-Byte beider Werte 0 ist). Der NRSC-Formatvorschlag
(§5.9) ist als **Entwurf, keine Quellenaussage** gekennzeichnet.

## Namenskorrektur gegenüber dem Auftrag

Der Auftrag nennt die Auswahlfunktion **„select_cicon"**. Dieses Symbol
existiert nicht in `references/emutos` (`grep -rn select_cicon
references/emutos/` → 0 Treffer). Die Funktion heisst **`best_match`**
(`gemrslib.c:267-282`); die Spec verwendet durchgehend den Quellnamen.

## Eigenständig getrieben, nicht aus dem Auftrag übernommen

* **`sdk/apps/teradesk/cicons.rsc`** (63 788 B, echte DRI-kompilierte
  Neuformat-Resource) mit Python `struct.unpack` gelesen, nicht angenommen:
  Header (`rsh_vrsn=4`, 39 Objekte, 1 Baum, `rsh_rssize=976`), das
  Erweiterungsfeld (`63788, 992, 61740, 0` — Index 2 wird von EmuTOS
  nirgends gelesen, Bedeutung offen gelassen statt geraten), die
  CICONBLK-Zeigertabelle (38 Nullen + Sentinel `-1`), drei CICONBLK-Datensaetze
  Byte fuer Byte (alle `ib_wicon=ib_hicon=32`, `num_planes=4`,
  `sel_data`-Flag gesetzt, `next_res=0`, 1608 B Abstand) und alle 39 Objekte
  (`ob_type`/`ob_spec`: Root `G_BOX`, 38× `G_CICON` mit `ob_spec=0..37`).
  Jede Zahl im neuen §5 stammt aus diesem Lauf, nicht aus dem Gedaechtnis.
* **RSC_CAP-Abschätzung** (§5.9): die rohen Pixel-/Maskendaten der 38 Icons
  allein sind 58 368 B; mit einer plausiblen Tabellen-Overhead-Schaetzung
  landet die native Fassung bei ≈ 62 000 B — unter dem aktuellen
  `RSC_CAP` von 65 536 B (`resource.rs:1481`), mit duennem ≈ 3,5-KB-Rand.
  Das widerspricht der in A38 notierten Annahme „farbige Icons brauchen mehr
  Platz" (`TASK/A38-STATUS-DONE.MD:78-80`) — eine Schaetzung, keine Messung
  an einem gebauten Konverter, aber mit echten Zahlen aus der Datei
  gerechnet, nicht geraten.
* **Der aktuelle VDI-Ruecksprung** (`docs/api/vdi/raster/_raster.md`) meldet
  heute **4 Planes** (16-Farb-CLUT), nicht 16 wie im Auftrag angenommen; der
  Truecolor-Umbau (`docs/analysis/vdi-audit/truecolor-conversion-plan.md`)
  ist eine eigene, noch nicht umgesetzte Entscheidung und erwaehnt CICON an
  keiner Stelle (0 Treffer fuer „cicon", gegrept). Beides ist in §5.7 als
  Vertragsfrage sauber getrennt: gegen das HEUTIGE Backend braucht A41 nur
  den Palettenpfad (jedes Icon in `cicons.rsc` trifft `best_match` exakt bei
  4 Planes, kein `expand_cicondata`, kein Truecolor-Zweig); der
  `phys_work.ext->palette[]`-Mechanismus wird erst relevant, falls/wenn der
  Truecolor-Umbau kommt — und selbst dann vermutlich nicht 1:1, weil
  NewTOS' eigener Truecolor-Plan bereits ein anderes Speicher-MFDB-Drahtformat
  vorsieht. Nicht aufgeloest, nur benannt.

## Was NICHT getan wurde (bewusst, Auftragsgrenze)

* Kein Code: `tools/mkrsc.py`, `tos-api/src/aes/resource.rs`,
  `tos-api/src/aes/object.rs`, `tools/make-sd-image.sh` — keine Datei
  angefasst. Die Bauliste (§5.10) beschreibt, was zu tun waere.
* Kein Host-/QEMU-Test gefahren (es gibt nichts zu testen — keine Zeile
  Produktcode geaendert).
* `docs/api/structures/iconblk.md` bewusst nicht angefasst — der Auftrag
  nennt nur `resource-fileselector.md` + `object-form.md` als Zieldateien;
  die dortige CICONBLK-Kurzbeschreibung bleibt stehen und widerspricht dem
  neuen §5 nicht (sie verweist bereits korrekt auf `just_draw`).

## Offene Entscheidungen für den Anwender (§5.11 der Spec, hier verdichtet)

1. **A39/A60-Reihenfolge.** TeraDesks `load_icons` laedt `cicons.rsc` **ueber
   die echte AES** und verlaesst sich danach darauf, dass `rsrc_gaddr`
   NewTOS' eigenem, noch nicht per-Prozess gefuehrtem `AESGLOBAL`-Selektor
   folgt (A39, offen) — dessen Ruecklesen wiederum auf LP64 A60 voraussetzt
   (`docs/analysis/rsrc-per-app-state-2026-09-19.md` §3/§4.3). A41 braucht
   A39 Stufe 2 (und damit A60) **oder** eine bewusste Zwischenloesung — welche,
   ist nicht meine Entscheidung.
2. **`RSC_CAP`.** Nach §5.9 vermutlich ausreichend, aber nur geschaetzt und
   mit duennem Rand — anheben/dynamisieren jetzt oder nach der echten
   Konverterzahl entscheiden: offen.
3. **NRSC v11/v12.** Ein Vorschlag, keine einzige moegliche Loesung —
   Abnahme durch den Anwender ausstehend, weil es neue Formatflaeche ist
   (anders als der Rest dieser Spec, der nur bestehendes Verhalten
   extrahiert).
4. **Truecolor-CICON-Pixelpfad.** EmuTOS' `phys_work.ext->palette[]`-Umweg
   ist vermutlich nicht das Ziel fuer einen kuenftigen NewTOS-Truecolor-Pfad,
   aber keine gelesene Quelle sagt, was stattdessen richtig waere — eigener
   Task, sobald/falls der Truecolor-Umbau eingeplant wird.

## Nächster Schritt

Bauliste §5.10 an den Developer: Konverter → Lader → Zeichner, in dieser
Reihenfolge, mit den dort genannten Host-/QEMU-Abnahmen. A41 bleibt **OFFEN**
in der Kopfzeile, bis diese Implementierung existiert und geprueft ist.
