import { call } from "@/lib/ipc";
import type { ServiceKind, ServiceStatus } from "@/types";

/** Mirrors lumina_services::install::Install. */
export interface Install {
  version: string;
  dir: string;
  exe: string;
}

/** Mirrors lumina_services::php_fcgi::PhpRuntime. */
export interface PhpRuntime {
  version: string;
  port: number;
  fellBack: boolean;
  children: number;
  maxRequests: number;
  ini: string;
}

/** Mirrors lumina_services::php_fcgi::PhpStatus. */
export interface PhpStatus {
  status: ServiceStatus;
  runtime: PhpRuntime | null;
  pid: number | null;
  processes: number;
  restarts: number;
  error: string | null;
  logFile: string;
}

export type WebServer = "nginx" | "apache";

/** Mirrors lumina_services::web::WebRuntime. */
export interface WebRuntime {
  server: WebServer;
  version: string;
  httpPort: number;
  phpPort: number;
}

/** Mirrors lumina_services::web::WebStatus. */
export interface WebStatus {
  server: WebServer;
  status: ServiceStatus;
  runtime: WebRuntime | null;
  pid: number | null;
  processes: number;
  error: string | null;
  logFile: string;
}

export type DatabaseEngine = "mysql" | "mariadb";

/** Mirrors lumina_services::database::DbRuntime. */
export interface DbRuntime {
  engine: DatabaseEngine;
  version: string;
  port: number;
  dataDir: string;
}

/** Mirrors lumina_services::database::DbStatus. */
export interface DbStatus {
  engine: DatabaseEngine;
  status: ServiceStatus;
  runtime: DbRuntime | null;
  pid: number | null;
  error: string | null;
  logFile: string;
}

/** Mirrors lumina_services::redis::RedisStatus. */
export interface RedisStatus {
  status: ServiceStatus;
  runtime: { version: string; port: number } | null;
  pid: number | null;
  error: string | null;
  logFile: string;
}

/** Mirrors lumina_services::orchestrator::StackStatus. */
export interface StackStatus {
  php: PhpStatus;
  web: WebStatus;
  database: DbStatus;
  redis: RedisStatus;
}

/** Mirrors lumina_services::web::ReloadOutcome. */
export type ReloadOutcome = { kind: "unchanged" } | { kind: "reloaded"; signals: number } | { kind: "restarted" };

export const SERVICES_KEY = ["services"] as const;
export const PHP_VERSIONS_KEY = ["php", "versions"] as const;
export const webVersionsKey = (server: WebServer) => ["web", "versions", server] as const;

export const webKind = (server: WebServer): ServiceKind => server;
export const dbKind = (engine: DatabaseEngine): ServiceKind => engine;
export const dbVersionsKey = (engine: DatabaseEngine) => ["db", "versions", engine] as const;
export const REDIS_VERSIONS_KEY = ["redis", "versions"] as const;

export const servicesStatus = () => call<StackStatus>("services_status");
export const serviceStart = (kind: ServiceKind) => call<void>("service_start", { kind });
export const serviceStop = (kind: ServiceKind) => call<void>("service_stop", { kind });
export const serviceRestart = (kind: ServiceKind) => call<void>("service_restart", { kind });
export const serviceReload = (kind: ServiceKind) => call<ReloadOutcome>("service_reload", { kind });
export const servicesStartAll = () => call<void>("services_start_all");
export const servicesStopAll = () => call<void>("services_stop_all");
export const serviceOpenLog = (kind: ServiceKind) => call<void>("service_open_log", { kind });
export const phpVersions = () => call<Install[]>("php_versions");
export const webVersions = (server: WebServer) => call<Install[]>("web_versions", { server });
export const dbVersions = (engine: DatabaseEngine) => call<Install[]>("db_versions", { engine });
export const redisVersions = () => call<Install[]>("redis_versions");
export const phpSetVersion = (version: string) => call<void>("php_set_version", { version });
export const openLocalhost = () => call<void>("open_localhost");
export const openWww = () => call<void>("open_www");
