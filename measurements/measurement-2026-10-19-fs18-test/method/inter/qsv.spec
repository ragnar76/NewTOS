# QSV -- (copied from measurement-2026-10-19-proc04-fdelete-test, name "qs.prg" instead of "qs.txt" so that QED's binary local options with backup=TRUE apply, options.c:921-925 -> back_up() Frename QS.PRG -> QS.BAK on the save-over, file.c:361-373)
# QED (#Z), armed up:QED, card qsv: Ctrl+N, type "abc", Ctrl+S -> save-as file selector, type "qs.prg", Return
# (Fcreate of a new file), type "d", Ctrl+S again (save over the existing C:\QS.TXT: QED save_datei -> Fcreate over it, or
# Frename to a backup first when its backup option is on, sdk/apps/qed/src/file.c:360-375, :525-540).
at 300
mkey 0x1D 0x31       # Ctrl+N
wait 100
mark q1-new
key 0x1E
key 0x30
key 0x2E
mark q2-typed
mkey 0x1D 0x1F       # Ctrl+S
wait 300
mark q3-saveas
key 0x10
key 0x1F
key 0x34
key 0x19             # p
key 0x13             # r
key 0x22             # g
mark q4-name
key 0x1C             # Return
wait 400
mark q5-saved
key 0x20             # d
mkey 0x1D 0x1F       # Ctrl+S: save over C:\QS.TXT
wait 400
mark q6-saved-over
wait 200
