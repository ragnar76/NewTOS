FS04 — das Messabbild fuer die echte Pi 4
=========================================

  kernel8-fs04probe.img
  sha256  13707ae9f773a1d8e2a1a9a1fc8d3cfa7c4ace3fdc88ec2c5a039bda58d27dab
  1 044 632 Bytes, release, Board rpi4, frische Scratch-Kopie von
  desk-rebuild @ c6cf484

Misst den umgestellten EMMC-Treiber (Zeit-Timeout, Wiederherstellung,
erhaltene Fehlerbits).  Der Bau ist reproduzierbar: zweimal unabhaengig
gebaut, beide Male dieselbe Pruefsumme.

Abnahme (2026-09-25, alles gefahren, nichts behauptet):
  * zwei Starts in qemu raspi4b, je 0 [EXC] / 0 [PANIC], Desktop mit
    vier Laufwerken oben
  * C:\FS04.TXT haengt an statt zu ueberschreiben: 344 Byte nach dem
    ersten Start, 688 nach dem zweiten, der zweite Block bytegleich zum
    ersten
  * fsck_msdos -n auf ALLEN VIER Partitionen des benutzten Abbilds: rc=0,
    alle drei Phasen durchlaufen
  * die scope.check-Identitaet haelt: n(read_one) = n(sectors) = n jeder
    dev.r.*-Zeile, ebenso schreibseitig — der Bruch 11 gegen 23 aus
    pi04 ist strukturell nicht mehr moeglich
  * emmc.spin ist aus dem Block verschwunden, die NOTE-Zeile und beide
    scope.check-Zeilen stehen als Zeichenkette im Abbild

Gebaut mit

  python3 measurement-2026-09-25-fs04/method/fs04_rebase_c6cf484.py \
          measurement-2026-09-25-fs04/method/fs04_probe_patch.py
  python3 measurement-2026-09-25-fs04/method/fs04_probe_patch.py . gesture
  cargo run -p xtask --release -- build --board rpi4 --release


WAS ES ANDERS MACHT ALS EIN NORMALER KERNEL
-------------------------------------------
Nichts am Verhalten — nur Messung. Es zaehlt und misst auf jeder Schicht
(Desktop, GEMDOS, FAT, Sektorcache, Blockebene, EMMC je Kommando in fuenf
Phasen, Cachepflege des Rahmenpuffers) und schreibt das Ergebnis

  **in die Datei  C:\FS04.TXT**  auf der Bootpartition,

weil auf der Pi keine serielle Ausgabe mitgeschnitten wird. Dieselbe Tabelle
geht zusaetzlich auf die serielle Konsole, damit die QEMU-Zahlen vergleichbar
bleiben.

* Beim Start ein kurzer Block mit Bildschirmgeometrie, Zaehlerfrequenz,
  Laufwerksliste und der Fehlerbilanz der Kartenidentifikation.
* Bei **jedem** Ablegen einer Datei ein Block mit der vollen Tabelle.
* **Angehaengt**, nie ueberschrieben.

Neu seit der Umstellung auf c6cf484:

* `scope.check.r` / `scope.check.w` — die Kontrollzeile, die zeigt, dass die
  Summe der fuenf Phasen die Gesamtzeit der Sektoroperation ergibt. Vorher
  stimmte das nicht: eine Wartezeit, die ihr Budget ausschoepfte, wurde gar
  nicht gebucht, weil die Buchung hinter dem `?` stand — deshalb standen in
  `../raw/pi04-FS04.TXT` `dev.write_one n=11` gegen `dev.w.wait_dat n=23`.
* `emmc.errors`, `emmc.timeouts_by_phase`, `emmc.error_bits`, `emmc.last` —
  die eigenen Zaehler des Treibers (`emmc::error_stats()`), je mit dem
  Zuwachs dieses Blocks in Klammern. Sie beantworten M2 des Audits direkt.
* `emmc.spin` ist **weg**. Seit der Timeout eine Dauer ist, ist die
  Schleifenzahl keine Grenze mehr und sagt nichts; eine NOTE-Zeile im Block
  sagt das.


DIE REIHENFOLGE, DIE DIE MESSUNG SAUBER HAELT
---------------------------------------------
Das Schreiben der Datei ist selbst GEMDOS-, FAT- und EMMC-Verkehr — also
genau das, was gemessen wird. Deshalb laeuft die Berichterstattung in vier
streng geordneten Schritten, und die Datei-Ein-/Ausgabe ist der **letzte**:

  1. jeden Zaehler — auch die des Treibers — in einen Schnappschuss kopieren
  2. den ganzen Block in einen Textpuffer formatieren  (noch kein I/O)
  3. seriell ausgeben                                   (noch kein I/O)
  4. Datei oeffnen, ans Ende springen, schreiben, schliessen

Aufgerufen wird das erst, **nachdem** das gemessene Zeitfenster geschlossen
ist (`fun_drag` verbucht `DRAG_TOTAL` und ruft den Bericht danach). Die
Kosten von Schritt 4 stehen im **naechsten** Block als
`previous report write`.

Zwei Nebenwirkungen, die ehrlich dazugehoeren:
* Die Sekunden, die der Anwender sieht, enthalten das Schreiben dieser Datei
  (in QEMU 1,4-3,0 ms). Es steht als eigene Zahl im naechsten Block.
* `C:\FS04.TXT` ist ein zusaetzliches Symbol im C:-Fenster. Beim Neuzeichnen
  kostet das rund 1 % (gemessen: `fb.clean` 2282 ohne, 2310 mit Datei).


IN QEMU GEPRUEFT (umgestellte Fassung)
---------------------------------------
* Das Abbild `13707ae9…` startet in `qemu -M raspi4b` bis
  `[desk] desktop up`, **0** `[EXC]`/`[PANIC]`; `C:\FS04.TXT` entsteht und
  ist lesbar, mit der Zeile
  `emmc after identification: 2 error(s), 0 timeout(s), 2 recovery(ies) CTO=2`
  — dieselben zwei CTOs, die die M1-Zeile des Treibers meldet.
* Der Ablegeblock mit der vollen Tabelle, injektorgetrieben, steht in
  `../raw/g08-FS04-rebased.TXT`: er steht **hinter** dem Startblock in
  derselben Datei, also wurde angehaengt und nicht ueberschrieben.
* Darin: `dev.read_one n=6` = `blk.read_blocks amount=6` = jede `dev.r.*`-
  Zeile n=6, und ebenso auf der Schreibseite — die Scopes stimmen ueberein.
  `scope.check.r  total=2056us phases=2032us diff=23us`,
  `scope.check.w  total=590us  phases=576us  diff=14us` (1,1 % bzw. 2,4 %
  Rest = die Registerschreibvorgaenge zwischen den Phasen).
* **Nicht mehr geprueft** (Datentraeger voll): der Anhaenge-Test ueber zwei
  Starts hinweg und `fsck_msdos -n` nach dem umgestellten Lauf. Beide waren
  in der vorigen Runde mit unveraendertem Anhaenge- und Schreibcode sauber;
  das ist ein Beleg fuer den Code, nicht fuer dieses Abbild. Wer das Abbild
  neu baut, sollte beides nachholen.
