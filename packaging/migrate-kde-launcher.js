// Preserve existing pins and their order; migrate only this application's old ID.
panels().forEach(function(panel) {
    panel.widgets().forEach(function(widget) {
        if (widget.type !== "org.kde.plasma.icontasks" && widget.type !== "org.kde.plasma.taskmanager") return;
        widget.currentConfigGroup = ["General"];
        var launchers = widget.readConfig("launchers", []);
        var changed = false;
        launchers = launchers.map(function(launcher) {
            if (launcher === "applications:vpn-manager.desktop") {
                changed = true;
                return "applications:br.com.pacheco.VpnManager.desktop";
            }
            return launcher;
        });
        if (changed) {
            widget.writeConfig("launchers", launchers);
            widget.reloadConfig();
        }
    });
});
