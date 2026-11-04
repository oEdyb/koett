# Windows DLL gate correction

Failed source: `1cd4d3f69c4fafa889e1fd0581483d20a39c7896`.
[Windows job 126806319215](https://github.com/oEdyb/koett/actions/runs/41895971835/job/126806319215)
compiled the executable, then failed the clean-Windows DLL check with:
`Windows executable requires non-system DLLs: uxtheme.dll`.

The native checkbox fix calls SetWindowTheme. Microsoft documents its DLL as
UxTheme.dll, supported since Windows Vista:
https://learn.microsoft.com/en-us/windows/win32/api/uxtheme/nf-uxtheme-setwindowtheme
The check's explicit system-DLL set omitted that Windows component. Add only
`uxtheme` to that set. Unknown/unbundled runtime dependencies remain failures.
No workflow, native runtime linking, supply-chain inventory or policy bypass
was introduced.

Rebased cleanly onto origin/main `16af788a3cbb9502f9ef234a349dbedd90f52074`.

Verification:

- The new UxTheme fixture failed before the correction for both regular and
  delayed imports; all six dependency tests pass afterward.
- Unknown DLLs, similarly named uxtheme_extra.dll, and VCRUNTIME140_1.dll are
  rejected for both regular and delayed imports.
- The original artifact 00e17c7 still fails the real PE check for
  vcruntime140.dll and vcruntime140_1.dll. The known static-CRT artifact e63e65a
  passes both its PE dependency check and the actual archive/notices check.
- Ten supply-chain inventory tests pass; generator --check and git diff --check
  pass. Inventory outputs remain current without regeneration.
- Guest status is powered off. No new Windows executable was run for this
  tooling-only correction. The rebuilt CI artifact must still pass the dark
  checkbox, UIA action/original-target guard, waveform, compact Settings,
  reduced-motion and tray refresh regressions before Milestone 1 is complete.
