import { useQuery } from "@tanstack/react-query";
import { FileCog, FileText, Play, Square } from "lucide-react";
import { editorOpen } from "@/features/env-path/api";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { InstallButton, NotInstalledBadge } from "@/components/Installable";
import { StatusBadge } from "@/components/StatusBadge";
import { Button, Fact, VersionSelect } from "@/components/ui";
import {
  POSTGRESQL_VERSIONS_KEY,
  type PgStatus,
  postgresqlVersions,
  serviceOpenLog,
  serviceRestart,
  serviceStart,
  serviceStop,
} from "@/features/services/api";
import type { PortConflictRequest } from "@/features/services/PortConflictDialog";
import { useServiceAction } from "@/features/services/useServiceAction";
import { type PostgresqlSettings, settingsUpdate } from "@/features/settings/api";
import { isCommandError, type PortConflict, portConflictOf } from "@/lib/ipc";

/** PostgreSQL: "include in Start All" switch, version picker, controls. */
export function PostgresCard(props: {
  status: PgStatus;
  config: PostgresqlSettings | undefined;
  onPortConflict: (request: PortConflictRequest) => void;
}) {
  const { t } = useTranslation();
  const { status, config } = props;
  const versions = useQuery({ queryKey: POSTGRESQL_VERSIONS_KEY, queryFn: postgresqlVersions });
  const action = useServiceAction();

  const rt = status.runtime;
  const running = rt !== null;
  const busy = action.isPending;
  const installed = versions.data ?? [];
  const missing = versions.isSuccess && installed.length === 0;

  const run = (fn: () => Promise<unknown>) =>
    action.mutate(fn, {
      onError: (err) => {
        const conflict = portConflictOf(err, config?.port);
        if (conflict && config) props.onPortConflict(postgresConflictRequest(conflict, config));
      },
    });

  const update = (patch: Partial<PostgresqlSettings>, restart = false) => {
    if (!config) return;
    run(async () => {
      await settingsUpdate("postgresql", { ...config, ...patch });
      if (restart && running) await serviceRestart("postgresql");
    });
  };

  const hideDialogError = isCommandError(action.error) && action.error.code === "port_in_use";

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="postgresql-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h2 id="postgresql-title" className="text-lg font-semibold">
            {t("postgresql.title")}
          </h2>
          {missing && !running ? <NotInstalledBadge /> : <StatusBadge status={busy ? (running ? "stopping" : "starting") : status.status} />}
        </div>
        <div className="flex flex-wrap gap-2">
          {running ? (
            <Button onClick={() => run(() => serviceStop("postgresql"))} disabled={busy} icon={<Square />}>
              {t("actions.stop")}
            </Button>
          ) : missing ? (
            <InstallButton pkg="postgresql" name="PostgreSQL" />
          ) : (
            <Button primary onClick={() => run(() => serviceStart("postgresql"))} disabled={busy || installed.length === 0} icon={<Play />}>
              {t("actions.start")}
            </Button>
          )}
          <Button onClick={() => run(() => editorOpen({ kind: "config", file: "postgresql" }))} disabled={busy} icon={<FileCog />}>
            {t("editor.editConfig")}
          </Button>
          <Button onClick={() => run(() => serviceOpenLog("postgresql"))} disabled={busy} icon={<FileText />}>
            {t("actions.openLog")}
          </Button>
        </div>
      </div>

      {config && (
        <label className="mt-4 flex items-center gap-2.5 text-sm">
          <input
            type="checkbox"
            className="size-4 accent-accent"
            checked={config.enabled}
            disabled={busy}
            onChange={(e) => update({ enabled: e.target.checked })}
          />
          {t("postgresql.enabled")}
        </label>
      )}

      {installed.length > 0 && config && (
        <div className="mt-4 max-w-sm">
          <VersionSelect
            label={t("postgresql.version")}
            newestLabel={t("postgresql.newest", { version: installed[0]?.version })}
            versions={installed.map((v) => v.version)}
            value={config.version}
            disabled={busy}
            onChange={(v) => update({ version: v }, true)}
          />
        </div>
      )}

      {versions.isSuccess && installed.length === 0 && (
        <p className="mt-4 text-sm text-muted">{t("postgresql.notInstalled")}</p>
      )}

      {rt && (
        <dl className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
          <Fact label={t("postgresql.version")} value={rt.version} />
          <Fact label={t("postgresql.port")} value={rt.port} />
          <Fact label={t("postgresql.user")} value="postgres" />
        </dl>
      )}
      {rt && <p className="mt-3 text-xs text-muted">{t("postgresql.connectHint", { port: rt.port })}</p>}

      <div className="mt-4 space-y-2">
        {!hideDialogError && <ErrorBox error={action.error} />}
        {!action.error && status.status === "failed" && status.error && (
          <ErrorBox error={{ code: "service", message: status.error }} />
        )}
      </div>
    </section>
  );
}

/** Moves PostgreSQL to `alternative`, then starts it. */
export function postgresConflictRequest(conflict: PortConflict, pg: PostgresqlSettings): PortConflictRequest {
  return {
    conflict,
    service: "PostgreSQL",
    isWeb: false,
    apply: async (alternative) => {
      await settingsUpdate("postgresql", { ...pg, port: alternative });
      await serviceStart("postgresql");
    },
  };
}
