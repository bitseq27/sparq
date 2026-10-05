:: 1. sanity: you should be clean at 6681f87 (the "Fresh start" head you published)
git status
git log --oneline -1

:: 2. pull the round in — fast-forward only, so it refuses loudly if your side diverged
git fetch <download-path>\sparq-handoff-2026-10-05-r8-final.bundle main
git merge --ff-only FETCH_HEAD        :: → 45c4826, seven commits with their messages intact

:: 3. re-stamp (until this, build.bat/test006 [00b] name the mismatch in words — expected)
python tools\sync_check.py --write --sync sparq-wo018-rebuild-2026-10-05
git add SYNC-STAMP.txt && git commit -m "sync stamp: sparq-wo018-rebuild-2026-10-05 (device)"

:: 4. prove it on the stage box, then deliver
scripts\gates.bat                     :: expect 925/0/1-ignored — full list in ROUND8-HANDOFF.md §7
git push.bat