// IPC wrappers for the `services` feature (see src-tauri/src/commands/services.rs).
import { commands } from "@/lib/bindings";
import type { DatabaseEngine, ServiceKind, WebServer } from "@/lib/bindings";

export type {
  DatabaseEngine,
  DbRuntime,
  DbStatus,
  Install,
  PhpRuntime,
  PgStatus,
  PhpStatus,
  RedisStatus,
  ReloadOutcome,
  StackStatus,
  WebRuntime,
  WebServer,
  WebStatus,
} from "@/lib/bindings";

export const SERVICES_KEY = ["services"] as const;
export const PHP_VERSIONS_KEY = ["php", "versions"] as const;
export const webVersionsKey = (server: WebServer) => ["web", "versions", server] as const;

export const webKind = (server: WebServer): ServiceKind => server;
export const dbKind = (engine: DatabaseEngine): ServiceKind => engine;
export const dbVersionsKey = (engine: DatabaseEngine) => ["db", "versions", engine] as const;
export const REDIS_VERSIONS_KEY = ["redis", "versions"] as const;
export const POSTGRESQL_VERSIONS_KEY = ["postgresql", "versions"] as const;

export const servicesStatus = () => commands.servicesStatus();
export const serviceStart = (kind: ServiceKind) => commands.serviceStart(kind);
export const serviceStop = (kind: ServiceKind) => commands.serviceStop(kind);
export const serviceRestart = (kind: ServiceKind) => commands.serviceRestart(kind);
export const serviceReload = (kind: ServiceKind) => commands.serviceReload(kind);
export const servicesStartAll = () => commands.servicesStartAll();
export const servicesStopAll = () => commands.servicesStopAll();
export const serviceOpenLog = (kind: ServiceKind) => commands.serviceOpenLog(kind);
export const phpVersions = () => commands.phpVersions();
export const webVersions = (server: WebServer) => commands.webVersions(server);
export const dbVersions = (engine: DatabaseEngine) => commands.dbVersions(engine);
export const redisVersions = () => commands.redisVersions();
export const postgresqlVersions = () => commands.postgresqlVersions();
export const phpSetVersion = (version: string) => commands.phpSetVersion(version);
export const openLocalhost = () => commands.openLocalhost();
export const openWww = () => commands.openWww();
