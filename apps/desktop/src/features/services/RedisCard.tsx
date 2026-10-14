import { useQuery } from "@tanstack/react-query";
import { FileCog, FileText, Play, Square } from "lucide-react";
import { editorOpen } from "@/features/env-path/api";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { StatusBadge } from "@/components/StatusBadge";
import { Button, Fact, VersionSelect } from "@/components/ui";
import {
  REDIS_VERSIONS_KEY,
  type RedisStatus,
  redisVersions,
  serviceOpenLog,
  serviceRestart,
  serviceStart,
  serviceStop,
} from "@/features/services/api";
import type { PortConflictRequest } from "@/features/services/PortConflictDialog";
import { useServiceAction } from "@/features/services/useServiceAction";
import { type RedisSettings, settingsUpdate } from "@/features/settings/api";
import { isCommandError, type PortConflict, portConflictOf } from "@/lib/ipc";

/** Redis: "include in Start All" switch, version picker, controls. */
export function RedisCard(props: {
  status: RedisStatus;
  config: RedisSettings | undefined;
  onPortConflict: (request: PortConflictRequest) => void;
}) {
  const { t } = useTranslation();
  const { status, config } = props;
  const versions = useQuery({ queryKey: REDIS_VERSIONS_KEY, queryFn: redisVersions });
  const action = useServiceAction();

  const rt = status.runtime;
  const running = rt !== null;
  const busy = action.isPending;
  const installed = versions.data ?? [];

  const run = (fn: () => Promise<unknown>) =>
    action.mutate(fn, {
      onError: (err) => {
        const conflict = portConflictOf(err, config?.port);
        if (conflict && config) props.onPortConflict(redisConflictRequest(conflict, config));
      },
    });

  const update = (patch: Partial<RedisSettings>, restart = false) => {
    if (!config) return;
    run(async () => {
      await settingsUpdate("redis", { ...config, ...patch });
      if (restart && running) await serviceRestart("redis");
    });
  };

  const hideDialogError = isCommandError(action.error) && action.error.code === "port_in_use";

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="redis-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h2 id="redis-title" className="text-lg font-semibold">
            {t("redis.title")}
          </h2>
          <StatusBadge status={busy ? (running ? "stopping" : "starting") : status.status} />
        </div>
        <div className="flex flex-wrap gap-2">
          {running ? (
            <Button onClick={() => run(() => serviceStop("redis"))} disabled={busy} icon={<Square />}>
              {t("actions.stop")}
            </Button>
          ) : (
            <Button primary onClick={() => run(() => serviceStart("redis"))} disabled={busy || installed.length === 0} icon={<Play />}>
              {t("actions.start")}
            </Button>
          )}
          <Button onClick={() => run(() => editorOpen({ kind: "config", file: "redis" }))} disabled={busy} icon={<FileCog />}>
            {t("editor.editConfig")}
          </Button>
          <Button onClick={() => run(() => serviceOpenLog("redis"))} disabled={busy} icon={<FileText />}>
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
          {t("redis.enabled")}
        </label>
      )}

      {installed.length > 0 && config && (
        <div className="mt-4 max-w-sm">
          <VersionSelect
            label={t("redis.version")}
            newestLabel={t("redis.newest", { version: installed[0]?.version })}
            versions={installed.map((v) => v.version)}
            value={config.version}
            disabled={busy}
            onChange={(v) => update({ version: v }, true)}
          />
        </div>
      )}

      {versions.isSuccess && installed.length === 0 && (
        <p className="mt-4 text-sm text-muted">
          {t("redis.notInstalled", { path: "bin/redis/<version>/", interpolation: { escapeValue: false } })}
        </p>
      )}

      {rt && (
        <dl className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
          <Fact label={t("redis.version")} value={rt.version} />
          <Fact label={t("redis.port")} value={rt.port} />
        </dl>
      )}

      <div className="mt-4 space-y-2">
        {!hideDialogError && <ErrorBox error={action.error} />}
        {!action.error && status.status === "failed" && status.error && (
          <ErrorBox error={{ code: "service", message: status.error }} />
        )}
      </div>
    </section>
  );
}

/** Moves Redis to `alternative`, then starts it. */
export function redisConflictRequest(conflict: PortConflict, redis: RedisSettings): PortConflictRequest {
  return {
    conflict,
    service: "Redis",
    isWeb: false,
    apply: async (alternative) => {
      await settingsUpdate("redis", { ...redis, port: alternative });
      await serviceStart("redis");
    },
  };
}
