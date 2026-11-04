#!/usr/bin/python3
"""Fixture window: logs real presses and active-state changes with times, and
holds a text view that receives pastes. Usage: lab-app.py TITLE LOGFILE"""
import sys, time, gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
title, path = sys.argv[1:3]
log = open(path, "a", buffering=1)
def note(text): log.write(f"{time.monotonic():.3f} {title} {text}\n")
def activate(app):
    win = Gtk.ApplicationWindow(application=app, title=title)
    view = Gtk.TextView()
    view.get_buffer().connect("changed", lambda b: note("TEXT " + repr(b.get_text(b.get_start_iter(), b.get_end_iter(), False))))
    click = Gtk.GestureClick(button=0)
    click.connect("pressed", lambda g, n, x, y: note(f"PRESS n={n} x={x:.0f} y={y:.0f}"))
    view.add_controller(click)
    win.set_child(view)
    win.connect("notify::is-active", lambda w, _: note("ACTIVE " + str(w.is_active())))
    win.present()
app = Gtk.Application(application_id="lab." + title.replace(" ", ""))
app.connect("activate", activate)
app.run([])
