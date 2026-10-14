import { call } from "@/lib/ipc";
import type { ServiceStatus } from "@/types";

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

/** Mirrors lumina_services::php_fcgi::PhpInstall. */
export interface PhpInstall {
  version: string;
  dir: string;
  cgi: string;
}

export const PHP_STATUS_KEY = ["php", "status"] as const;
export const PHP_VERSIONS_KEY = ["php", "versions"] as const;

export const phpStatus = () => call<PhpStatus>("php_status");
export const phpVersions = () => call<PhpInstall[]>("php_versions");
export const phpStart = () => call<PhpRuntime>("php_start");
export const phpStop = () => call<void>("php_stop");
export const phpRestart = () => call<PhpRuntime>("php_restart");
export const phpSetVersion = (version: string) => call<PhpStatus>("php_set_version", { version });
export const phpOpenLog = () => call<void>("php_open_log");
