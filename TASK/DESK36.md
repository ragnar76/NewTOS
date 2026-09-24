# DESK36 — Beim Ziehen wird das Ziel unter dem Zeiger nicht hervorgehoben

Status: **OFFEN** · Prioritaet: **P2** · Typ: Belegter Defekt — Pi-Runde 2026-09-24

Abhaengigkeiten: —

## Sachverhalt

Zieht man eine Datei ueber ein anderes Symbol (Programm, Ordner, Papierkorb,
Laufwerk), wird das Ziel **nicht** invertiert. Die Logik dahinter stimmt — das
Ablegen landet am richtigen Ziel (auf der Pi mit `CMDLINE.NCP` geprueft) —,
aber der Benutzer sieht beim Ziehen nicht, worauf er trifft.

EmuTOS macht das in der Ziehschleife selbst, `gr_drgplns`
(`references/emutos/desk/deskact.c:251-360`). Sie fuehrt ueber die ganze
Bewegung einen Zustand `curr_wh`/`curr_root`/`curr_sel` mit:

```c
*pdobj = gr_obfind(tree, root, l_mx, l_my);      /* :309  was liegt unter dem Zeiger */

if ((*pdobj == root) || (*pdobj == NIL)) {        /* :313  nichts mehr drunter */
    if (curr_sel) { act_chg(curr_wh, curr_root, curr_sel, pc, FALSE, TRUE); ... }
}
if (*pdobj != curr_sel) {                         /* :325  Ziel gewechselt */
    if (curr_sel) { act_chg(..., FALSE, TRUE); ... }
}
obj = tree + *pdobj;
if (!(obj->ob_state & SELECTED)) {                /* :335 */
    pa = i_find(dst_wh, *pdobj, NULL, &isapp);
    if (pa) {
        if ((pa->a_type == AT_ISFILE) && !isapp)  /* :340  "don't highlight plain files" */
            continue;
        curr_sel = *pdobj;
        act_chg(curr_wh, curr_root, curr_sel, pc, TRUE, TRUE);
    }
}
...
if (curr_sel)                                     /* :353  beim Loslassen zuruecknehmen */
    act_chg(curr_wh, curr_root, curr_sel, pc, FALSE, TRUE);
```

Drei Regeln stecken darin, die ein naiver Nachbau verfehlt:

1. **Eine gewoehnliche Datei wird nicht hervorgehoben** — nur Ordner,
   Laufwerke, Papierkorb, Drucker und *installierte Anwendungen* (`isapp`).
2. Beim Wechsel des Ziels wird das alte **erst zurueckgenommen**, dann das
   neue gesetzt; ein bereits `SELECTED`-Objekt wird gar nicht angefasst.
3. Beim Loslassen wird die Hervorhebung **immer** zurueckgenommen, auch wenn
   das Ablegen scheitert.

`kernel/src/desk/act.rs` hat `act_chg`, aber kein Gegenstueck zu
`gr_drgplns`' Zustandsfuehrung: der Zeiger zieht ueber die Symbole, ohne dass
eines seinen Zustand aendert.

## Arbeit

`gr_drgplns`' Schleife nachziehen. Vorher am Quelltext feststellen, wo unsere
Ziehschleife liegt und ob sie ueberhaupt pro Bewegung laeuft oder nur beim
Loslassen auswertet — davon haengt ab, ob das eine Ergaenzung oder ein Umbau
ist. `i_find` gibt es seit DESK19 als `dir::i_find_win`.

Abnahme: eine Datei ueber Programm, Ordner, Papierkorb und eine gewoehnliche
Datei ziehen. Die ersten drei muessen invertieren, die vierte nicht; beim
Verlassen geht die Hervorhebung zurueck, beim Loslassen ebenfalls.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
