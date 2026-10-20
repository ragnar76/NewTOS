# QFI -- QED (#Z), armed up:QED, card qfi-<v>: C:\Apps\QED\DATED.TXT carries the FAT stamp 2026-10-17 12:00:00 (mkcard touch -t).
# Ctrl+O, "dated.txt", Return (the file selector starts in QED's folder, measurement-2026-10-20-sdk01a-test qre: QS.PRG landed in
# C:\APPS\QED); Ctrl+I = Datei > Info... (menu "Info...  ^I") -> QED's Textinfo dialog shows Datum/Uhrzeit from QED file_time() =
# stat().st_mtime + localtime (sdk/apps/qed/src/global.c:346-370, edit.c:2320). Return closes it. Then "d", Ctrl+S (save over: QED
# compares file_time() with the time it stored at load, file.c:569-583 -- before SDK01 F both were 0; now a mismatch would raise the
# MOVED2 alert), "e", Ctrl+S again (stored time = file_time() after the first save, file.c:604), Ctrl+I: Datum/Uhrzeit of the save.
at 300
mkey 0x1D 0x18       # Ctrl+O -> file selector
wait 400
mark q1-fsel
text dated.txt
mark q2-name
key 0x1C             # Return -> open
wait 500
mark q3-opened
mkey 0x1D 0x17       # Ctrl+I -> Textinfo
wait 400
mark q4-info
key 0x1C             # Return -> close the dialog
wait 300
mark q5-info-closed
key 0x20             # d
mkey 0x1D 0x1F       # Ctrl+S: save over
wait 500
mark q6-saved
key 0x12             # e
mkey 0x1D 0x1F       # Ctrl+S again
wait 500
mark q7-saved2
mkey 0x1D 0x17       # Ctrl+I -> Textinfo after the saves
wait 400
mark q8-info2
key 0x1C
wait 300
mark q9-end
wait 200
