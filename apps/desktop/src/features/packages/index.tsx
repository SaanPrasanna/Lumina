import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { Check, Download, Play, RotateCw, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui";
import {
  type AvailableVersion,
  formatBytes,
  type Job,
  PACKAGES_KEY,
  type PackagesOverview,
  type PackageView,
  packageCancel,
  packageDismiss,
  packageInstall,
  packagesList,
  packageSetActive,
  packageUninstall,
  type ProgressEvent,
} from "@/features/packages/api";

/** Query keys that show installed versions somewhere in the app. */
const AFFECTED = [PACKAGES_KEY, ["services"], ["php"], ["web"], ["db"], ["redis"], ["sites"]] as const;

/** Live progress from `package-progress`, by `id@version`. */
function useProgress() {
  const [progress, setProgress] = useState<Record<string, ProgressEvent>>({});
  useEffect(() => {
    const unlisten = listen<ProgressEvent>("package-progress", (e) =>
      setProgress((p) => ({ ...p, [`${e.payload.id}@${e.payload.version}`]: e.payload })),
    );
    return () => void unlisten.then((fn) => fn());
  }, []);
  return progress;
}

/** Quick Add: install, switch and remove versions of every tool. */
export function PackagesPage() {
  const { t } = useTranslation();
  const list = useQuery({ queryKey: PACKAGES_KEY, queryFn: packagesList });
  const [tab, setTab] = useState("php");
  const progress = useProgress();
  const data = list.data;
  const current = data?.packages.find((p) => p.id === tab) ?? data?.packages[0];

  return (
    <>
      <PageHeader title={t("nav.packages")} />
      <div className="space-y-4 px-8 pb-8">
        <p className="-mt-2 text-sm text-muted">{t("packages.intro")}</p>
        <ErrorBox error={list.error} />
        {data && (
          <>
            <div role="tablist" aria-label={t("nav.packages")} className="flex flex-wrap gap-1.5">
              {data.packages.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  role="tab"
                  aria-selected={p.id === current?.id}
                  onClick={() => setTab(p.id)}
                  className={`rounded-md px-3 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent ${
                    p.id === current?.id ? "bg-accent text-on-accent" : "border border-line hover:bg-hover"
                  }`}
                >
                  {p.name}
                  {p.installed.length > 0 && (
                    <span className={`ms-1.5 text-xs ${p.id === current?.id ? "text-on-accent/80" : "text-muted"}`}>
                      {p.installed.length}
                    </span>
                  )}
                </button>
              ))}
            </div>
            {current && <PackagePanel key={current.id} pkg={current} overview={data} progress={progress} />}
            {data.catalogUpdated && (
              <p className="text-xs text-muted">{t("packages.catalogDate", { date: data.catalogUpdated })}</p>
            )}
          </>
        )}
      </div>
    </>
  );
}

function usePackageAction() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () => Promise.all(AFFECTED.map((queryKey) => queryClient.invalidateQueries({ queryKey }))),
  });
}

function PackagePanel(props: { pkg: PackageView; overview: PackagesOverview; progress: Record<string, ProgressEvent> }) {
  const { t } = useTranslation();
  const { pkg, overview } = props;
  const action = usePackageAction();
  const busy = action.isPending;
  const isDbUi = pkg.id === "phpmyadmin";

  return (
    <section role="tabpanel" aria-labelledby={`tab-${pkg.id}`} className="space-y-4 rounded-xl border border-line p-5">
      <div>
        <h2 id={`tab-${pkg.id}`} className="text-lg font-semibold">
          {pkg.name}
        </h2>
        <p className="mt-1 text-sm text-muted">{pkg.description}</p>
        <p className="mt-2 text-sm">
          {pkg.active
            ? t(pkg.running ? "packages.activeRunning" : "packages.active", { version: pkg.active })
            : t("packages.noneActive")}
          {pkg.selected === "" && pkg.installed.length > 0 && (
            <span className="text-muted"> · {t(isDbUi ? "packages.autoDbUi" : "packages.autoNewest")}</span>
          )}
        </p>
        {isDbUi && overview.phpVersion && (
          <p className={`mt-1 text-sm ${overview.dbUi ? "text-muted" : "text-warn"}`}>
            {overview.dbUi
              ? t("packages.dbUiServed", { version: overview.dbUi, php: overview.phpVersion })
              : pkg.installed.length > 0
                ? t("packages.dbUiNone", { php: overview.phpVersion })
                : t("packages.dbUiInstall")}
          </p>
        )}
      </div>

      <ErrorBox error={action.error} />

      {pkg.installed.length > 0 && (
        <div>
          <h3 className="mb-2 text-sm font-semibold">{t("packages.installed")}</h3>
          <ul className="divide-y divide-line rounded-lg border border-line">
            {pkg.installed.map((v) => {
              const active = pkg.active === v.version;
              return (
                <li key={v.version} className="flex flex-wrap items-center justify-between gap-3 px-4 py-2.5">
                  <div className="flex flex-wrap items-center gap-2 text-sm">
                    <span className="font-mono font-medium" dir="ltr">
                      {v.version}
                    </span>
                    {active && <Badge tone="ok">{pkg.running === v.version ? t("packages.running") : t("packages.inUse")}</Badge>}
                    {pkg.selected === v.version && <Badge>{t("packages.pinned")}</Badge>}
                    {!v.managed && <Badge>{t("packages.manual")}</Badge>}
                    {v.requiresPhp && <span className="text-xs text-muted">PHP {v.requiresPhp}</span>}
                  </div>
                  <div className="flex flex-wrap gap-2">
                    {pkg.selected !== v.version && (
                      <Button onClick={() => action.mutate(() => packageSetActive(pkg.id, v.version))} disabled={busy} icon={<Play />}>
                        {t("packages.use")}
                      </Button>
                    )}
                    <Button
                      onClick={() => action.mutate(() => packageUninstall(pkg.id, v.version))}
                      disabled={busy || v.inUse !== null}
                      title={v.inUse ? t(`packages.inUseReason.${v.inUse}`) : undefined}
                      icon={<Trash2 />}
                    >
                      {t("packages.remove")}
                    </Button>
                  </div>
                </li>
              );
            })}
          </ul>
          {pkg.selected !== "" && (
            <button
              type="button"
              className="mt-2 text-sm text-accent hover:underline disabled:opacity-50"
              disabled={busy}
              onClick={() => action.mutate(() => packageSetActive(pkg.id, ""))}
            >
              {t(isDbUi ? "packages.useAutoDbUi" : "packages.useNewest")}
            </button>
          )}
        </div>
      )}

      <div>
        <h3 className="mb-2 text-sm font-semibold">{t("packages.available")}</h3>
        {pkg.available.length === 0 ? (
          <p className="text-sm text-muted">{t("packages.noneForPlatform")}</p>
        ) : (
          <ul className="divide-y divide-line rounded-lg border border-line">
            {pkg.available.map((v) => (
              <AvailableRow
                key={v.version}
                pkg={pkg}
                release={v}
                job={pkg.jobs.find((j) => j.version === v.version)}
                live={props.progress[`${pkg.id}@${v.version}`]}
                showCompat={isDbUi && overview.phpVersion !== null}
              />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function AvailableRow(props: {
  pkg: PackageView;
  release: AvailableVersion;
  job: Job | undefined;
  live: ProgressEvent | undefined;
  showCompat: boolean;
}) {
  const { t } = useTranslation();
  const { pkg, release, job } = props;
  const action = usePackageAction();
  const running = job !== undefined && !job.done;
  // The event stream is fresher than the last list fetch.
  const stage = (running && props.live?.stage) || job?.stage || null;
  const received = running && props.live ? props.live.received : (job?.received ?? 0);
  const total = (running && props.live?.total) || job?.total || release.size;
  const percent = total ? Math.min(100, Math.round((received / total) * 100)) : null;

  return (
    <li className="px-4 py-2.5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <span className="font-mono font-medium" dir="ltr">
            {release.version}
          </span>
          {release.channel && <Badge>{release.channel}</Badge>}
          {release.size !== null && <span className="text-xs text-muted">{formatBytes(release.size)}</span>}
          {release.requiresPhp && <span className="text-xs text-muted">PHP {release.requiresPhp}</span>}
          {props.showCompat && !release.compatible && <Badge tone="warn">{t("packages.incompatible")}</Badge>}
        </div>
        <div className="flex flex-wrap gap-2">
          {release.installed ? (
            <span className="flex items-center gap-1 text-sm text-ok">
              <Check className="size-4" /> {t("packages.isInstalled")}
            </span>
          ) : running ? (
            <Button onClick={() => action.mutate(() => packageCancel(pkg.id, release.version))} icon={<X />}>
              {t("actions.cancel")}
            </Button>
          ) : (
            <Button
              primary
              onClick={() => action.mutate(() => packageInstall(pkg.id, release.version))}
              disabled={action.isPending}
              icon={release.partial ? <RotateCw /> : <Download />}
            >
              {release.partial ? t("packages.resume") : t("packages.install")}
            </Button>
          )}
        </div>
      </div>

      {running && (
        <div className="mt-2" role="status">
          <div className="h-1.5 overflow-hidden rounded-full bg-hover" aria-hidden="true">
            <div
              className={`h-full bg-accent transition-[width] ${percent === null || stage !== "downloading" ? "w-full animate-pulse" : ""}`}
              style={percent !== null && stage === "downloading" ? { width: `${percent}%` } : undefined}
            />
          </div>
          <p className="mt-1 text-xs text-muted">
            {stage === "downloading" || stage === null
              ? t("packages.downloading", {
                  done: formatBytes(received),
                  total: total ? formatBytes(total) : "?",
                })
              : t(`packages.stage.${stage}`)}
          </p>
        </div>
      )}

      {job?.done && job.error && (
        <div className="mt-2 flex items-start justify-between gap-3 rounded-md bg-danger-soft px-3 py-2 text-sm text-danger">
          <span className="min-w-0 break-words">
            {job.error === "cancelled" ? t("packages.cancelled") : t("packages.failed", { error: job.error, interpolation: { escapeValue: false } })}
          </span>
          <button
            type="button"
            aria-label={t("packages.dismiss")}
            className="shrink-0 rounded p-0.5 hover:bg-danger/10"
            onClick={() => action.mutate(() => packageDismiss(pkg.id, release.version))}
          >
            <X className="size-4" />
          </button>
        </div>
      )}
      <ErrorBox error={action.error} />
    </li>
  );
}

function Badge({ children, tone }: { children: React.ReactNode; tone?: "ok" | "warn" }) {
  const colour = tone === "ok" ? "bg-ok/15 text-ok" : tone === "warn" ? "bg-warn-soft text-warn" : "bg-hover text-muted";
  return <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${colour}`}>{children}</span>;
}
