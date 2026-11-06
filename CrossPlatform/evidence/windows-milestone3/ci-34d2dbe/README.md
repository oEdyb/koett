# M3 native History — real Windows

Source 34d2dbe2275bd408370c1cd64aff48583e2d1363; CI run 42006324312.
EXE SHA256 3fc6c1fa59b28b784dfc88e98aa8351f98fe5c5ee3ae5def34ac94beb226872a.
Private Windows 11 guest, 4 vCPU/8 GiB, host initially 30 GiB available.
Only the isolated guest was controlled. No host input or application data used.

The script returned passed:true. A 110-row SQLite fixture temporarily replaced
only guest history after backing it up. Exactly 50 rows were visible initially.

- Final Copy: M3 final 108 ÅÄÖ; raw TogglePattern then Copy: M3 raw 108 åäö.
- Single export matched raw Copy exactly.
- Export results wrote all 110 rows, including row 000, without Load more.
  The window reported Exported 110 transcripts.
- Search 108 exported one matching raw row.
- Save dialog Cancel preserved the prior export hash.
- Row 109 had no raw text; raw view/Copy correctly returned its final text.

Native Copy and checkbox used UIA; export commands used WM_COMMAND, followed
by actual guest typing into the native Save filename field and UIA Save/Cancel.
The driver's initial ValuePattern.SetValue changed the displayed filename but
was not committed by the shell dialog; keyboard entry fixed the harness. No
product fix was needed for these successful checks. Screenshots and foreground
HWND/PID accompany each step; result.json records assertions and export hash.

Guest history was restored with SHA256
9411f0bc77566cc565f53961acbfe748d20442a51f4d0fff4a2e6957163b6427.
Windows fully shut down afterward; transfer server stopped.

This proves History at the guest's 200% scale/dark theme, not complete M3.
New Words UI has only metadata compilation; dictionary/replacement editing,
import/export and new Fix a word plus later formatting/model-switch checks await its
rebuilt artifact. New Words layout/accessibility/theme/scaling remains open.
No physical microphone, big benchmark, release or M3 merge was performed.
