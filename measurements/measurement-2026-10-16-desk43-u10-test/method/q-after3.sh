#!/usr/bin/env bash
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
until grep -q OLD2-DONE /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/after2.log; do sleep 5; done
bash "$HERE/pair2.sh" km /private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/6484c0fd-61c5-4c60-a3ef-ede2401f4ca7/scratchpad/cards/plain.img km on 'No desktop: NEWDESK@1=6:fallback.png' 'No desktop: NEWDESK@1=25:end.png'; echo KM-DONE
