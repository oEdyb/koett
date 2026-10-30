#!/usr/bin/env python3
"""Run the native UI regression against current three-shortcut Settings labels."""
from pathlib import Path

source = Path(__file__).with_name('test_linux_ui.py')
text = source.read_text()
old = 'wait(lambda: any(node_role(node) == "label" and "Text" in node.get_interfaces() and Atspi.Text.get_text(node, 0, -1) == "Managed by desktop" for node in walk(settings_window)), "desktop-managed shortcut label")'
new = '''wait(lambda: all(find(name, "label", settings_window) for name in ["Dictation", "Ask", "Media"])
                 and sum(node_role(node) == "label" and node.get_name() == "Not assigned" for node in walk(settings_window)) == 3,
                 "three unassigned private desktop shortcut labels")'''
assert text.count(old) == 1
exec(compile(text.replace(old, new), str(source), 'exec'), {'__name__': '__main__', '__file__': str(source)})  # noqa: S102 - executes the adjacent tracked regression harness
