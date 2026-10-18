import { useQuery } from "@tanstack/react-query";
import { Database, FileCog, FileText, Play, RotateCw, Square } from "lucide-react";
import { editorOpen } from "@/features/env-path/api";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { InstallButton, NotInstalledBadge } from "@/components/Installable";
import { StatusBadge } from "@/components/StatusBadge";
import { Button, Fact, VersionSelect } from "@/components/ui";
import {
  type DatabaseEngine,
  type DbStatus,
  dbKind,
  dbVersions,
  dbVersionsKey,
  serviceOpenLog,
  serviceRestart,
  serviceStart,
  serviceStop,
} from "@/features/services/api";
import type { PortConflictRequest } from "@/features/services/PortConflictDialog";
import { useServiceAction } from "@/features/services/useServiceAction";
import { dbUiOpen } from "@/features/packages/api";
import { adminerOpen } from "@/features/services/api";
import { type DatabaseSettings, settingsUpdate } from "@/features/settings/api";
import { isCommandError, type PortConflict, portConflictOf } from "@/lib/ipc";

const ENGINE_NAMES: Record<DatabaseEngine, string> = { mysql: "MySQL", mariadb: "MariaDB" };

/** MySQL or MariaDB: engine and version pickers, facts, controls. */
export function DatabaseCard(props: {
  status: DbStatus;
  config: DatabaseSettings | undefined;
  onPortConflict: (request: PortConflictRequest) => void;
}) {
  const { t } = useTranslation();
  const { status, config } = props;
  const engine: DatabaseEngine = config?.engine ?? status.engine;
  const versions = useQuery({ queryKey: dbVersionsKey(engine), queryFn: () => dbVersions(engine) });
  const action = useServiceAction();

  const rt = status.runtime;
  const running = rt !== null;
  const busy = action.isPending;
  const installed = versions.data ?? [];
  const missing = versions.isSuccess && installed.length === 0;
  const kind = dbKind(engine);
  const versionKey = engine === "mysql" ? "mysql_version" : "mariadb_version";

  const run = (fn: () => Promise<unknown>) =>
    action.mutate(fn, {
      onError: (err) => {
        const conflict = portConflictOf(err, config?.port);
        if (conflict && config) props.onPortConflict(dbConflictRequest(conflict, config));
      },
    });

  const update = (patch: Partial<DatabaseSettings>) => {
    if (!config) return;
    run(async () => {
      await settingsUpdate("database", { ...config, ...patch });
      // While running: a new engine is started (the backend stops the other
      // first); a new version needs a restart.
      if (!running) return;
      if (patch.engine && patch.engine !== engine) {
        await serviceStart(dbKind(patch.engine));
      } else {
        await serviceRestart(kind);
      }
    });
  };

  const hideDialogError = isCommandError(action.error) && action.error.code === "port_in_use";

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="db-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h2 id="db-title" className="text-lg font-semibold">
            {t("db.title")}
          </h2>
          {missing && !running ? <NotInstalledBadge /> : <StatusBadge status={busy ? (running ? "stopping" : "starting") : status.status} />}
        </div>
        <div className="flex flex-wrap gap-2">
          {running ? (
            <>
              <Button onClick={() => run(() => serviceRestart(kind))} disabled={busy} icon={<RotateCw />}>
                {t("actions.restart")}
              </Button>
              <Button onClick={() => run(() => serviceStop(kind))} disabled={busy} icon={<Square />}>
                {t("actions.stop")}
              </Button>
            </>
          ) : missing ? (
            <InstallButton pkg={kind} name={engine === "mysql" ? "MySQL" : "MariaDB"} />
          ) : (
            <Button primary onClick={() => run(() => serviceStart(kind))} disabled={busy || installed.length === 0} icon={<Play />}>
              {t("actions.start")}
            </Button>
          )}
          <Button onClick={() => run(dbUiOpen)} disabled={busy} icon={<Database />}>
            {t("db.openUi")}
          </Button>
          <Button onClick={() => run(adminerOpen)} disabled={busy} icon={<Database />}>
            {t("db.openAdminer")}
          </Button>
          <Button onClick={() => run(() => editorOpen({ kind: "config", file: "database" }))} disabled={busy} icon={<FileCog />}>
            {t("editor.editConfig")}
          </Button>
          <Button onClick={() => run(() => serviceOpenLog(kind))} disabled={busy} icon={<FileText />}>
            {t("actions.openLog")}
          </Button>
        </div>
      </div>

      <div className="mt-4 flex max-w-xl flex-wrap gap-3">
        <label className="flex min-w-0 flex-1 flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("db.engine")}</span>
          <select
            className="rounded-md border border-line bg-surface px-3 py-2"
            value={engine}
            disabled={busy || !config}
            onChange={(e) => update({ engine: e.target.value as DatabaseEngine })}
          >
            <option value="mysql">MySQL</option>
            <option value="mariadb">MariaDB</option>
          </select>
        </label>
        {installed.length > 0 && config && (
          <VersionSelect
            label={t("db.version")}
            newestLabel={t("db.newest", { version: installed[0]?.version })}
            versions={installed.map((v) => v.version)}
            value={config[versionKey]}
            disabled={busy}
            onChange={(v) => update({ [versionKey]: v })}
          />
        )}
      </div>

      {versions.isSuccess && installed.length === 0 && (
        <p className="mt-4 text-sm text-muted">
          {t("db.notInstalled", {
            engine: ENGINE_NAMES[engine],
            path: `bin/${engine}/<version>/`,
            interpolation: { escapeValue: false },
          })}
        </p>
      )}

      {rt ? (
        <dl className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
          <Fact label={t("db.version")} value={`${ENGINE_NAMES[rt.engine]} ${rt.version}`} />
          <Fact label={t("db.port")} value={rt.port} />
          <div className="col-span-2 min-w-0">
            <Fact label={t("db.dataDir")} value={<span className="block truncate">{rt.dataDir}</span>} />
          </div>
        </dl>
      ) : (
        installed.length > 0 && <p className="mt-4 text-xs text-muted">{t("db.firstStart")}</p>
      )}
      <p className="mt-3 text-xs text-muted">{t("db.login")}</p>

      <div className="mt-4 space-y-2">
        {!hideDialogError && <ErrorBox error={action.error} />}
        {!action.error && status.status === "failed" && status.error && (
          <ErrorBox error={{ code: "service", message: status.error }} />
        )}
      </div>
    </section>
  );
}

/** Moves the database to `alternative`, then starts it. */
export function dbConflictRequest(conflict: PortConflict, db: DatabaseSettings): PortConflictRequest {
  return {
    conflict,
    service: ENGINE_NAMES[db.engine],
    isWeb: false,
    apply: async (alternative) => {
      await settingsUpdate("database", { ...db, port: alternative });
      await serviceStart(dbKind(db.engine));
    },
  };
}
