# QSV -- QED (#Z), armed up:QED, card qsv: Ctrl+N, type "abc", Ctrl+S -> save-as file selector, type "qs.txt", Return
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
key 0x14
key 0x2D
key 0x14
mark q4-name
key 0x1C             # Return
wait 400
mark q5-saved
key 0x20             # d
mkey 0x1D 0x1F       # Ctrl+S: save over C:\QS.TXT
wait 400
mark q6-saved-over
wait 200
