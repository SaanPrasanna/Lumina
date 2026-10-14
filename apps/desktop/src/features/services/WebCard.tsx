import { useQuery } from "@tanstack/react-query";
import { FileCog, FileText, Play, RefreshCw, RotateCw, Square } from "lucide-react";
import { editorOpen } from "@/features/env-path/api";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { StatusBadge } from "@/components/StatusBadge";
import { Button, Fact, VersionSelect } from "@/components/ui";
import {
  type ReloadOutcome,
  serviceOpenLog,
  serviceReload,
  serviceRestart,
  serviceStart,
  serviceStop,
  type WebServer,
  type WebStatus,
  webKind,
  webVersions,
  webVersionsKey,
} from "@/features/services/api";
import { useServiceAction } from "@/features/services/useServiceAction";
import { settingsUpdate, type WebSettings } from "@/features/settings/api";
import type { PortConflictRequest } from "@/features/services/PortConflictDialog";
import { isCommandError, type PortConflict, portConflictOf } from "@/lib/ipc";

/** Web server (nginx or Apache): server and version pickers, facts, controls. */
export function WebCard(props: {
  status: WebStatus;
  config: WebSettings | undefined;
  onPortConflict: (request: PortConflictRequest) => void;
}) {
  const { t } = useTranslation();
  const { status, config } = props;
  const server: WebServer = config?.server ?? status.server;
  const versions = useQuery({ queryKey: webVersionsKey(server), queryFn: () => webVersions(server) });
  const action = useServiceAction();
  const [reloadOutcome, setReloadOutcome] = useState<ReloadOutcome | null>(null);

  const rt = status.runtime;
  const running = rt !== null;
  const busy = action.isPending;
  const installed = versions.data ?? [];
  const kind = webKind(server);
  const versionKey = server === "nginx" ? "nginx_version" : "apache_version";

  /** Runs an action; a busy HTTP port opens the port-conflict dialog. */
  const run = (fn: () => Promise<unknown>) => {
    setReloadOutcome(null);
    action.mutate(fn, {
      onError: (err) => {
        const conflict = portConflictOf(err, config?.http_port) ?? portConflictOf(err, config?.https_port);
        if (conflict && config) props.onPortConflict(webConflictRequest(conflict, config));
      },
    });
  };

  const updateWeb = (patch: Partial<WebSettings>) => {
    if (!config) return;
    run(async () => {
      await settingsUpdate("web", { ...config, ...patch });
      // Applied immediately while running: a new server is started (the
      // backend stops the old one first); a new version needs a restart.
      if (!running) return;
      if (patch.server && patch.server !== server) {
        await serviceStart(webKind(patch.server));
      } else {
        await serviceRestart(kind);
      }
    });
  };

  const hideDialogError = isCommandError(action.error) && action.error.code === "port_in_use";

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="web-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h2 id="web-title" className="text-lg font-semibold">
            {t("web.title")}
          </h2>
          <StatusBadge status={busy ? (running ? "stopping" : "starting") : status.status} />
        </div>
        <div className="flex flex-wrap gap-2">
          {running ? (
            <>
              <Button
                onClick={() => run(async () => setReloadOutcome(await serviceReload(kind)))}
                disabled={busy}
                icon={<RefreshCw />}
              >
                {t("actions.reload")}
              </Button>
              <Button onClick={() => run(() => serviceRestart(kind))} disabled={busy} icon={<RotateCw />}>
                {t("actions.restart")}
              </Button>
              <Button onClick={() => run(() => serviceStop(kind))} disabled={busy} icon={<Square />}>
                {t("actions.stop")}
              </Button>
            </>
          ) : (
            <Button primary onClick={() => run(() => serviceStart(kind))} disabled={busy || installed.length === 0} icon={<Play />}>
              {t("actions.start")}
            </Button>
          )}
          <Button onClick={() => run(() => editorOpen({ kind: "config", file: "webSites" }))} disabled={busy} icon={<FileCog />}>
            {t("editor.editSites")}
          </Button>
          <Button onClick={() => run(() => serviceOpenLog(kind))} disabled={busy} icon={<FileText />}>
            {t("actions.openLog")}
          </Button>
        </div>
      </div>

      <div className="mt-4 flex max-w-xl flex-wrap gap-3">
        <label className="flex min-w-0 flex-1 flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("web.server")}</span>
          <select
            className="rounded-md border border-line bg-surface px-3 py-2"
            value={server}
            disabled={busy || !config}
            onChange={(e) => updateWeb({ server: e.target.value as WebServer })}
          >
            <option value="nginx">nginx</option>
            <option value="apache">Apache</option>
          </select>
        </label>
        {installed.length > 0 && config && (
          <VersionSelect
            label={t("web.version")}
            newestLabel={t("web.newest", { version: installed[0]?.version })}
            versions={installed.map((v) => v.version)}
            value={config[versionKey]}
            disabled={busy}
            onChange={(v) => updateWeb({ [versionKey]: v })}
          />
        )}
      </div>

      {versions.isSuccess && installed.length === 0 && (
        <p className="mt-4 text-sm text-muted">
          {t("web.notInstalled", {
            server: server === "nginx" ? "nginx" : "Apache",
            path: `bin/${server}/<version>/`,
            interpolation: { escapeValue: false },
          })}
        </p>
      )}

      {rt && (
        <dl className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
          <Fact label={t("web.version")} value={`${rt.server === "nginx" ? "nginx" : "Apache"} ${rt.version}`} />
          <Fact label={t("web.port")} value={rt.httpPort} />
          <Fact label={t("web.phpPort")} value={rt.phpPort} />
          <Fact label={t("web.processes")} value={status.processes} />
        </dl>
      )}

      {reloadOutcome && running && (
        <p role="status" className="mt-3 text-sm text-ok">
          {t(`web.${reloadOutcome.kind}`)}
        </p>
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

/**
 * Moves the busy port to `alternative`, then starts the server. For the HTTP
 * port, HTTPS also moves from 443 to 8443; for the HTTPS port only HTTPS moves.
 */
export function webConflictRequest(conflict: PortConflict, web: WebSettings): PortConflictRequest {
  const https = conflict.port === web.https_port && conflict.port !== web.http_port;
  return {
    conflict,
    service: web.server === "nginx" ? "nginx" : "Apache",
    isWeb: !https,
    apply: async (alternative) => {
      await settingsUpdate(
        "web",
        https
          ? { ...web, https_port: alternative }
          : { ...web, http_port: alternative, https_port: web.https_port === 443 ? 8443 : web.https_port },
      );
      await serviceStart(webKind(web.server));
    },
  };
}
