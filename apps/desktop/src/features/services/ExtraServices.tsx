import { useQuery } from "@tanstack/react-query";
import { ExternalLink, FileText, Play, Square } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { InstallButton, NotInstalledBadge } from "@/components/Installable";
import { StatusBadge } from "@/components/StatusBadge";
import { Button, Fact, VersionSelect } from "@/components/ui";
import { EXTRAS_KEY, type ExtraStatus, extraOpenLog, extraOpenUi, extraRestart, extraStart, extraStop, extrasStatus } from "@/features/services/api";
import { useServiceAction } from "@/features/services/useServiceAction";
import { type AppConfig, settingsUpdate } from "@/features/settings/api";

/** MongoDB, Valkey, Meilisearch, Typesense, SeaweedFS: one compact card each. */
export function ExtraServices({ config }: { config: AppConfig | undefined }) {
  const { t } = useTranslation();
  const extras = useQuery({ queryKey: EXTRAS_KEY, queryFn: extrasStatus });
  const [showAll, setShowAll] = useState(false);
  if (!extras.data || !config) return null;
  // Installed or running ones always; the rest behind "Show all".
  const shown = extras.data.filter((e) => showAll || e.versions.length > 0 || e.runtime);
  const hidden = extras.data.length - shown.length;

  return (
    <section aria-labelledby="extras-title" className="space-y-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 id="extras-title" className="text-sm font-semibold text-muted">
          {t("extras.title")}
        </h2>
        {hidden > 0 && (
          <button type="button" className="rounded text-sm text-accent-ink outline-none hover:underline focus-visible:ring-2 focus-visible:ring-accent" onClick={() => setShowAll(true)}>
            {t("extras.showAll", { count: hidden })}
          </button>
        )}
      </div>
      {shown.length === 0 && <p className="text-sm text-muted">{t("extras.none")}</p>}
      {shown.map((extra) => (
        <ExtraCard key={extra.id} extra={extra} config={config} />
      ))}
    </section>
  );
}

function ExtraCard({ extra, config }: { extra: ExtraStatus; config: AppConfig }) {
  const { t } = useTranslation();
  const action = useServiceAction();
  const running = extra.runtime !== null;
  const busy = action.isPending;
  const installed = extra.versions.length > 0;

  const update = (patch: Partial<{ enabled: boolean; version: string; port: number }>, restart = false) =>
    action.mutate(async () => {
      const current = config.services[extra.id] ?? { enabled: false, version: "", port: 0 };
      await settingsUpdate("services", { ...config.services, [extra.id]: { ...current, ...patch } });
      if (restart && running) await extraRestart(extra.id);
    });

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby={`extra-${extra.id}`}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h3 id={`extra-${extra.id}`} className="text-lg font-semibold">
            {extra.name}
          </h3>
          {!installed && !running ? <NotInstalledBadge /> : <StatusBadge status={busy ? (running ? "stopping" : "starting") : extra.status} />}
        </div>
        <div className="flex flex-wrap gap-2">
          {running ? (
            <Button onClick={() => action.mutate(() => extraStop(extra.id))} disabled={busy} icon={<Square />}>
              {t("actions.stop")}
            </Button>
          ) : installed ? (
            <Button primary onClick={() => action.mutate(() => extraStart(extra.id))} disabled={busy} icon={<Play />}>
              {t("actions.start")}
            </Button>
          ) : (
            <InstallButton pkg={extra.id} name={extra.name} />
          )}
          {extra.webUi && (
            <Button onClick={() => action.mutate(() => extraOpenUi(extra.id))} disabled={busy || !running} icon={<ExternalLink />}>
              {t("extras.openUi")}
            </Button>
          )}
          <Button onClick={() => action.mutate(() => extraOpenLog(extra.id))} disabled={busy} icon={<FileText />}>
            {t("actions.openLog")}
          </Button>
        </div>
      </div>
      {installed ? (
        <div className="mt-4 flex flex-wrap items-end gap-6">
          <label className="flex items-center gap-2.5 text-sm">
            <input type="checkbox" className="size-4 accent-accent" checked={extra.enabled} disabled={busy} onChange={(e) => update({ enabled: e.target.checked })} />
            {t("extras.enabled")}
          </label>
          <div className="w-64">
            <VersionSelect
              label={t("extras.version")}
              newestLabel={t("extras.newest", { version: extra.versions[0] })}
              versions={extra.versions}
              value={extra.version}
              disabled={busy}
              onChange={(v) => update({ version: v }, true)}
            />
          </div>
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="font-medium">{t("extras.port")}</span>
            <input
              type="number"
              min={1}
              max={65535}
              dir="ltr"
              className="w-28 rounded-md border border-line bg-surface px-3 py-1.5 font-mono"
              defaultValue={extra.port}
              disabled={busy}
              onBlur={(e) => {
                const port = Number(e.target.value);
                if (port > 0 && port !== extra.port) update({ port }, true);
              }}
            />
          </label>
        </div>
      ) : (
        <p className="mt-3 text-sm text-muted">{t("extras.notInstalled", { name: extra.name })}</p>
      )}
      {extra.runtime && (
        <dl className="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-4">
          <Fact label={t("extras.version")} value={extra.runtime.version} />
          <Fact label={t("extras.port")} value={extra.runtime.port} />
        </dl>
      )}
      <p className="mt-3 font-mono text-xs text-muted" dir="ltr">
        {extra.connect}
      </p>
      <div className="mt-3 space-y-2 empty:hidden">
        <ErrorBox error={action.error} />
        {!action.error && extra.status === "failed" && extra.error && <ErrorBox error={{ code: "service", message: extra.error }} />}
      </div>
    </section>
  );
}
