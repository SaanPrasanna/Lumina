/** Mirrors lumina_core::types (serde kebab-case). */
export type ServiceKind = "php-fcgi" | "nginx" | "apache" | "mysql" | "mariadb" | "redis";
export type ServiceStatus = "stopped" | "starting" | "running" | "stopping" | "failed";

/** Mirrors lumina_config::validate::Warning. */
export interface ConfigWarning {
  code: string;
  field: string;
  message: string;
  values: Record<string, string>;
}
