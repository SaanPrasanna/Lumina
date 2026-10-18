import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FileText, Play, RotateCw, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { StatusBadge } from "@/components/StatusBadge";
import { Button } from "@/components/ui";
import { type AppStatus, appOpenLog, appRestart, appStart, appStop, type Site, SITES_KEY } from "@/features/sites/api";
import { type SiteOverride } from "@/features/settings/api";
import { isCommandError } from "@/lib/ipc";

/** The settings a site has when nothing is configured for it. */
export const DEFAULT_OVERRIDE: SiteOverride = { enabled: true, docroot: "", kind: "auto", command: "", port: 0, autostart: false, php: "" };

/** `override` with every field, for sites saved before a field existed. */
export function fullOverride(override: Partial<SiteOverride> | undefined): SiteOverride {
  return { ...DEFAULT_OVERRIDE, ...override };
}

/** Whether `override` changes nothing, so `[sites.<name>]` can be left out. */
export function isDefaultOverride(override: SiteOverride): boolean {
  return (Object.keys(DEFAULT_OVERRIDE) as (keyof SiteOverride)[]).every((k) => override[k] === DEFAULT_OVERRIDE[k]);
}

/** A site's dev server: framework, port, state and its controls. */
export function DevServer({ site, status }: { site: Site; status: AppStatus | undefined }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const action = useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () => queryClient.invalidateQueries({ queryKey: SITES_KEY }),
  });
  const app = site.app;
  if (!app) return null;
  const state = status?.status ?? "stopped";
  const running = state === "running";
  const busy = action.isPending;
  const noCommand = app.command.trim() === "";

  return (
    <div className="mt-3 rounded-md border border-line px-3 py-2.5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-sm">
          <span className="font-medium">{t("sites.app.title", { framework: app.framework })}</span>
          <StatusBadge status={busy ? (running ? "stopping" : "starting") : state} />
          <span className="font-mono text-xs text-muted" dir="ltr">
            {t("sites.app.address", { port: app.port })}
          </span>
        </div>
        <div className="flex flex-wrap gap-2">
          {running ? (
            <>
              <Button onClick={() => action.mutate(() => appStop(site.name))} disabled={busy} icon={<Square />}>
                {t("actions.stop")}
              </Button>
              <Button onClick={() => action.mutate(() => appRestart(site.name))} disabled={busy} icon={<RotateCw />} aria-label={t("actions.restart")} title={t("actions.restart")} />
            </>
          ) : (
            <Button primary onClick={() => action.mutate(() => appStart(site.name))} disabled={busy || noCommand} icon={<Play />}>
              {t("sites.app.start")}
            </Button>
          )}
          <Button onClick={() => action.mutate(() => appOpenLog(site.name))} disabled={busy} icon={<FileText />}>
            {t("actions.openLog")}
          </Button>
        </div>
      </div>
      <p className="mt-1.5 truncate font-mono text-xs text-muted" dir="ltr" title={app.command}>
        {noCommand ? t("sites.app.noCommand") : app.command}
      </p>
      {status && status.runtimes.length > 0 && (
        <p className="mt-0.5 text-xs text-muted">{t("sites.app.runtimes", { list: status.runtimes.join(", "), interpolation: { escapeValue: false } })}</p>
      )}
      <div className="mt-2 space-y-2 empty:hidden">
        {action.error && !(isCommandError(action.error) && action.error.code === "port_in_use") && <ErrorBox error={action.error} />}
        {action.error && isCommandError(action.error) && action.error.code === "port_in_use" && (
          <ErrorBox error={{ code: "port_in_use", message: t("sites.app.portBusy", { port: app.port }) }} />
        )}
        {!action.error && state === "failed" && status?.error && <ErrorBox error={{ code: "service", message: status.error }} />}
      </div>
    </div>
  );
}

/** How the site is served, plus the dev server's command, port and Start All switch. */
export function ServedByFields(props: {
  site: Site;
  value: SiteOverride;
  onChange: (next: SiteOverride) => void;
}) {
  const { t } = useTranslation();
  const { site, value, onChange } = props;
  const detected = site.app && !site.app.configured ? site.app.framework : null;
  const asApp = value.kind === "app" || (value.kind === "auto" && (detected !== null || value.command.trim() !== ""));
  const input = "rounded-md border border-line bg-surface px-3 py-1.5 font-mono";
  return (
    <>
      <label className="flex flex-col gap-1.5 text-sm">
        <span className="font-medium">{t("sites.app.servedBy")}</span>
        <select className="rounded-md border border-line bg-surface px-3 py-1.5" value={value.kind} onChange={(e) => onChange({ ...value, kind: e.target.value as SiteOverride["kind"] })}>
          <option value="auto">{detected ? t("sites.app.autoDetected", { framework: detected }) : t("sites.app.autoPhp")}</option>
          <option value="php">{t("sites.app.php")}</option>
          <option value="app">{t("sites.app.devServer")}</option>
        </select>
      </label>
      {asApp && (
        <>
          <label className="flex min-w-64 flex-1 flex-col gap-1.5 text-sm">
            <span className="font-medium">{t("sites.app.command")}</span>
            <input
              className={input}
              dir="ltr"
              placeholder={detected ? t("sites.app.commandDetected") : t("sites.app.commandPlaceholder")}
              value={value.command}
              onChange={(e) => onChange({ ...value, command: e.target.value })}
            />
          </label>
          <label className="flex w-32 flex-col gap-1.5 text-sm">
            <span className="font-medium">{t("sites.app.port")}</span>
            <input
              type="number"
              min={0}
              max={65535}
              className={input}
              dir="ltr"
              placeholder={String(site.app?.port ?? "")}
              value={value.port === 0 ? "" : value.port}
              onChange={(e) => onChange({ ...value, port: Number(e.target.value) || 0 })}
            />
          </label>
          <label className="flex items-center gap-2.5 self-center text-sm">
            <input type="checkbox" className="size-4 accent-accent" checked={value.autostart} onChange={(e) => onChange({ ...value, autostart: e.target.checked })} />
            {t("sites.app.autostart")}
          </label>
        </>
      )}
    </>
  );
}
