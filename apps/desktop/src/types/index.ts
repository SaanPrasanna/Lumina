/** Mirrors lumina_core::types (serde kebab-case). Keep in sync or generate with tauri-specta. */
export type ServiceKind = "php-fcgi" | "nginx" | "apache" | "mysql" | "mariadb" | "redis";
export type ServiceStatus = "stopped" | "starting" | "running" | "stopping" | "failed";
