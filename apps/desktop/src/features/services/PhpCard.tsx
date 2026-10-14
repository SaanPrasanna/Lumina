import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileText, Play, RotateCw, Square } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { StatusBadge } from "@/components/StatusBadge";
import {
  PHP_STATUS_KEY,
  PHP_VERSIONS_KEY,
  phpOpenLog,
  phpRestart,
  phpSetVersion,
  phpStart,
  phpStatus,
  phpStop,
  phpVersions,
} from "@/features/services/api";
import { SETTINGS_KEY, settingsGet } from "@/features/settings/api";

/** PHP FastCGI: status, version switcher, runtime facts and controls. */
export function PhpCard() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const status = useQuery({
    queryKey: PHP_STATUS_KEY,
    queryFn: phpStatus,
    // Worker counts change without events (e.g. workers recycling); poll while up.
    refetchInterval: (q) => (q.state.data?.status === "running" ? 5000 : false),
  });
  const versions = useQuery({ queryKey: PHP_VERSIONS_KEY, queryFn: phpVersions });
  const settings = useQuery({ queryKey: SETTINGS_KEY, queryFn: settingsGet });

  const refresh = () => queryClient.invalidateQueries({ queryKey: ["php"] });
  const action = useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: refresh,
  });
  const setVersion = useMutation({ mutationFn: phpSetVersion, onSettled: refresh });

  const s = status.data;
  const rt = s?.runtime;
  const installed = versions.data ?? [];
  const configured = settings.data?.config.php;
  const busy = action.isPending || setVersion.isPending;
  const running = s?.status === "running" || s?.status === "starting";

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="php-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h2 id="php-title" className="text-lg font-semibold">
            {t("php.title")}
          </h2>
          {s && <StatusBadge status={busy ? (running ? "stopping" : "starting") : s.status} />}
        </div>

        <div className="flex flex-wrap gap-2">
          {running ? (
            <>
              <Button onClick={() => action.mutate(phpRestart)} disabled={busy} icon={<RotateCw />}>
                {t("actions.restart")}
              </Button>
              <Button onClick={() => action.mutate(phpStop)} disabled={busy} icon={<Square />}>
                {t("actions.stop")}
              </Button>
            </>
          ) : (
            <Button
              primary
              onClick={() => action.mutate(phpStart)}
              disabled={busy || installed.length === 0}
              icon={<Play />}
            >
              {t("actions.start")}
            </Button>
          )}
          <Button onClick={() => action.mutate(phpOpenLog)} disabled={busy} icon={<FileText />}>
            {t("actions.openLog")}
          </Button>
        </div>
      </div>

      {versions.isSuccess && installed.length === 0 && (
        <p className="mt-4 text-sm text-muted">
          {t("php.notInstalled", { path: "bin/php/<version>/", interpolation: { escapeValue: false } })}
        </p>
      )}

      {installed.length > 0 && (
        <label className="mt-4 flex max-w-sm flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("php.version")}</span>
          <select
            className="rounded-md border border-line bg-surface px-3 py-2"
            value={configured?.version ?? ""}
            disabled={busy || !configured}
            onChange={(e) => setVersion.mutate(e.target.value)}
          >
            <option value="">{t("php.newest", { version: installed[0]?.version })}</option>
            {installed.map((v) => (
              <option key={v.version} value={v.version}>
                PHP {v.version}
              </option>
            ))}
          </select>
        </label>
      )}

      {rt && s && (
        <dl className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5">
          <Fact label={t("php.version")} value={rt.version} />
          <Fact
            label={t("php.port")}
            value={
              <span className="flex items-center gap-2">
                {rt.port}
                {rt.fellBack && (
                  <span
                    className="rounded bg-warn-soft px-1.5 py-0.5 text-xs text-warn"
                    title={t("php.fallbackHint", { configured: configured?.fcgi_port, port: rt.port })}
                  >
                    {t("php.fallback")}
                  </span>
                )}
              </span>
            }
          />
          <Fact label={t("php.processes")} value={s.processes} />
          <Fact label={t("php.children")} value={rt.children} />
          <Fact
            label={t("php.maxRequests")}
            value={rt.maxRequests === 0 ? t("php.unlimited") : rt.maxRequests}
          />
        </dl>
      )}

      {rt?.fellBack && (
        <p className="mt-3 text-sm text-warn">
          {t("php.fallbackHint", { configured: configured?.fcgi_port, port: rt.port })}
        </p>
      )}

      <div className="mt-4 space-y-2">
        <ErrorBox error={action.error ?? setVersion.error} />
        {!action.error && s?.status === "failed" && s.error && (
          <ErrorBox error={{ code: "service", message: s.error }} />
        )}
      </div>
    </section>
  );
}

function Fact({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="min-w-0">
      <dt className="truncate text-xs text-muted">{label}</dt>
      <dd className="mt-0.5 font-mono text-sm" dir="ltr">
        {value}
      </dd>
    </div>
  );
}

function Button({
  children,
  icon,
  primary,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { icon: ReactNode; primary?: boolean }) {
  return (
    <button
      type="button"
      {...props}
      className={`flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-50 [&_svg]:size-4 ${
        primary ? "bg-accent text-white hover:bg-accent-strong" : "border border-line hover:bg-hover"
      }`}
    >
      {icon}
      {children}
    </button>
  );
}
