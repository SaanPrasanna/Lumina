Linux packaging notes: `.deb`, `.rpm`, AppImage are produced by Tauri.
Binding to ports 80/443 requires `setcap 'cap_net_bind_service=+ep'` on the nginx/httpd
binaries (done via lumina-helper) or using unprivileged ports.

`dev.lumina.app.helper.policy` is the polkit action for `lumina-helper` (hosts
file changes run through `pkexec`). Install it to `/usr/share/polkit-1/actions/`
and keep its `exec.path` equal to the installed helper's path. Without it pkexec
still works, with the generic "run as the super user" prompt.
