# M3 native Words — real Windows

Source c5a243b0744bef9d4393776034ffd02e80fd5e41; CI run 42010706336.
ZIP SHA256 16f8c91d0c63fad264376f152e0e1a3ad6d09a12f15da3cdb6497e514041141d.
EXE SHA256 5cc22ed0f269a2d40a8ccc69f9471be92f2008bd0143350a6a2262823de01a88.
Private 4-vCPU/8-GiB Windows 11 guest at nice 19; host 36 GiB available at boot.
Both scripts returned passed:true; all nine cases passed:

- Explicit term Save persisted two sounds-like forms.
- An 81-character invalid term showed a size error and left settings unchanged.
- Literal replacement Save preserved the dictionary; Remove removed only that rule.
- Native import Cancel changed nothing; acceptance merged two UTF-8 terms.
- Malformed plain text (= invalid form) showed the exact line error with no write.
- Export produced canonical plain text including ÅTerm and its sounds-like form.
- Read-only settings produced Access denied visibly and preserved the saved file.

Settings content hash and original read-only attribute were restored. No host
input or application data was used. Screenshots and foreground HWND/PID are
included; raw results omit images and record each assertion.

Native single-line fields used UIA ValuePattern. Multiline controls expose
TextPattern and were edited by actual guest keyboard input. Category selection,
Save and Remove used native UIA patterns. File dialogs used the native File name
mnemonic and keyboard, then native commands and UIA confirmation. The initial
JSON-invalid test was invalid itself: the shared exchange parser is plain text,
not JSON. The Windows picker incorrectly offered JSON; the newer source narrows
it to plain text to match Mac. That filter change awaits its own artifact.

These receipts do not prove the newer correction/proposal workflow, model and
formatting controls, or M3 completion. Those are newer Windows-only candidates.
No experimental acoustic bias pass, large benchmark, release or M3 merge.
