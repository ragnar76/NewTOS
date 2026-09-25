FS04 — das Messabbild fuer die echte Pi 4
=========================================

  kernel8-fs04probe.img
  sha256  53bbbeab356e30a4bfe994f86fcdcda964c1fe29b85f8a9d0ddfbb0dc5fc762e
  1 036 376 Bytes, gebaut 2026-09-25

Gebaut aus einer frischen Scratch-Kopie von `desk-rebuild` @ `31fef0f`, mit

  python3 measurement-2026-09-25-fs04/method/fs04_probe_patch.py . gesture
  cargo run -p xtask --release -- build --board rpi4 --release

Also **release**, Board **rpi4**. Der gemeinsame Baum wurde dafuer nicht
angefasst; der Patch steht als Skript und als Diff in `../method/`.


WAS ES ANDERS MACHT ALS EIN NORMALER KERNEL
-------------------------------------------
Nichts am Verhalten — nur Messung. Es zaehlt und misst auf jeder Schicht
(Desktop, GEMDOS, FAT, Sektorcache, Blockebene, EMMC je Kommando und je
Abfrageschleife, Cachepflege des Rahmenpuffers) und schreibt das Ergebnis

  **in die Datei  C:\FS04.TXT**  auf der Bootpartition,

weil auf der Pi keine serielle Ausgabe mitgeschnitten wird. Dieselbe Tabelle
geht zusaetzlich auf die serielle Konsole, damit die QEMU-Zahlen vergleichbar
bleiben.

* Beim Start schreibt es einen kurzen Block mit Bildschirmgeometrie,
  Zaehlerfrequenz und Laufwerksliste.
* Bei **jedem** Ablegen einer Datei (Ziehen auf ein Laufwerkssymbol oder in
  ein Fenster) haengt es einen Block mit der vollen Tabelle an.
* Es wird **angehaengt**, nie ueberschrieben — mehrere Versuche stehen
  hintereinander in derselben Datei.

Die Datei ist reiner Text mit CR/LF und laesst sich am Mac direkt oeffnen.


DIE REIHENFOLGE, DIE DIE MESSUNG SAUBER HAELT
---------------------------------------------
Das Schreiben der Datei ist selbst GEMDOS-, FAT- und EMMC-Verkehr — also genau
das, was gemessen wird. Deshalb laeuft die Berichterstattung in vier streng
geordneten Schritten, und die Datei-Ein-/Ausgabe ist der **letzte**:

  1. jeden Zaehler aus den Atomics in einen Schnappschuss kopieren
  2. den ganzen Block in einen Textpuffer formatieren  (noch kein I/O)
  3. seriell ausgeben                                   (noch kein I/O)
  4. Datei oeffnen, ans Ende springen, schreiben, schliessen

Aufgerufen wird das erst, **nachdem** das gemessene Zeitfenster geschlossen
ist (`fun_drag` verbucht `DRAG_TOTAL` und ruft den Bericht danach). Die Kosten
von Schritt 4 werden gemessen und im **naechsten** Block als
`previous report write` ausgewiesen — sie verschwinden also nicht, sie stehen
nur nicht in den Zahlen, die sie verfaelschen wuerden.

Zwei Nebenwirkungen, die ehrlich dazugehoeren:
* Die Sekunden, die der Anwender sieht, enthalten das Schreiben dieser Datei
  (in QEMU 1,4-3,0 ms). Es steht als eigene Zahl im naechsten Block.
* `C:\FS04.TXT` ist ein zusaetzliches Symbol im C:-Fenster. Beim Neuzeichnen
  kostet das rund 1 % mehr (gemessen: `fb.clean` 2282 ohne, 2310 mit Datei).


IN QEMU GEPRUEFT, BEVOR ES HIER LIEGT
--------------------------------------
* Genau dieses Abbild startet in `qemu -M raspi4b` bis `[desk] desktop up`,
  0 `[EXC]`/`[PANIC]` (`../raw/pi03-boot-of-handover-image.log`).
* `C:\FS04.TXT` entsteht und ist lesbar (Startblock, 268 Byte).
* Zweiter Start auf derselben Karte: der Block wird **angehaengt** (536 Byte),
  nicht ueberschrieben.
* Der Ablegeblock mit der vollen Tabelle wurde mit der injektorgetriebenen
  Variante desselben Patches geprueft (identischer Berichtscode, einziger
  Unterschied ist der IKBD-Injektor, den das Uebergabeabbild NICHT enthaelt —
  auf der Pi zieht der Anwender selbst). Ergebnis woertlich:
  `../raw/g07-FS04.TXT`.
* `fsck_msdos -n` auf C: und D: nach allen Laeufen: sauber.
