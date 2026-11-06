tell application "Finder"
 tell disk "Koett Layout"
  open
  set current view of container window to icon view
  set toolbar visible of container window to false
  set statusbar visible of container window to false
  set bounds of container window to {120, 120, 660, 450}
  set options to icon view options of container window
  set arrangement of options to not arranged
  set icon size of options to 96
  set text size of options to 13
  set position of item "Koett.app" to {140, 145}
  set position of item "Applications" to {400, 145}
  update without registering applications
  close
  open
 end tell
end tell
