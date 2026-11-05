function place(window) {
    if (window.caption === 'niri' || window.resourceClass === 'niri') {
        window.fullScreen = true;
        workspace.activeWindow = window;
    }
}
workspace.windowAdded.connect(place);
for (const window of workspace.windowList()) place(window);
