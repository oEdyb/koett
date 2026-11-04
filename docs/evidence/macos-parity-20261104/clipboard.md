# Drawing clipboard acceptance

PR #51: isolated NSPasteboard tests passed (8 tests), full suite passed
(398 tests, 9 skipped), release build passed. Test directories and pasteboards
were separate from app data. No microphone recording or user transcript was used.

A standalone Swift fixture called the production SketchClipboard implementation
with a generated 3×3 PNG and `Koett repeat paste <literal> & svenska 👋`.
Headed Google Chrome was launched by Playwright in an isolated temporary profile.
Two real Meta+V presses into a contenteditable rich editor each exposed
`text/plain`, `text/html`, and a PNG File (150 bytes). The final editor held two
images and two exact copies of the text. HTML contained a base64 data URL and
escaped `<literal>` and `&`. Headless Chrome exposed an empty platform clipboard;
that run is not evidence of native paste compatibility. The original clipboard
was restored only after checking that the fixture still owned it.

The installed T3 Router 0.1.25 preview could not generate a paste event through
preview keyboard calls. The current T3 source's composerImagePasteText.ts and
ComposerPromptEditorTiptap.tsx accept image+text when the HTML has body text;
Koett's HTML satisfies that contract. This is source evidence only. Live paste
into the actual T3 chat box remains open. No message was sent and no existing
chat draft was changed.
