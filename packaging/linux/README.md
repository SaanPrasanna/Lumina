Linux packaging notes: `.deb`, `.rpm`, AppImage are produced by Tauri.
Binding to ports 80/443 requires `setcap 'cap_net_bind_service=+ep'` on the nginx/httpd
binaries (done via lumina-helper) or using unprivileged ports.
