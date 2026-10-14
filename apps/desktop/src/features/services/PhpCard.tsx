import { useQuery } from "@tanstack/react-query";
import { FileText, Play, RotateCw, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { StatusBadge } from "@/components/StatusBadge";
import { Button, Fact, VersionSelect } from "@/components/ui";
import {
  PHP_VERSIONS_KEY,
  type PhpStatus,
  phpSetVersion,
  phpVersions,
  serviceOpenLog,
  serviceRestart,
  serviceStart,
  serviceStop,
} from "@/features/services/api";
import { useServiceAction } from "@/features/services/useServiceAction";
import type { PhpSettings } from "@/features/settings/api";

/** PHP FastCGI: status, version switcher, runtime facts and controls. */
export function PhpCard({ status, config }: { status: PhpStatus; config: PhpSettings | undefined }) {
  const { t } = useTranslation();
  const versions = useQuery({ queryKey: PHP_VERSIONS_KEY, queryFn: phpVersions });
  const action = useServiceAction();

  const rt = status.runtime;
  const installed = versions.data ?? [];
  const busy = action.isPending;
  const running = rt !== null;

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="php-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h2 id="php-title" className="text-lg font-semibold">
            {t("php.title")}
          </h2>
          <StatusBadge status={busy ? (running ? "stopping" : "starting") : status.status} />
        </div>
        <div className="flex flex-wrap gap-2">
          {running ? (
            <>
              <Button onClick={() => action.mutate(() => serviceRestart("php-fcgi"))} disabled={busy} icon={<RotateCw />}>
                {t("actions.restart")}
              </Button>
              <Button onClick={() => action.mutate(() => serviceStop("php-fcgi"))} disabled={busy} icon={<Square />}>
                {t("actions.stop")}
              </Button>
            </>
          ) : (
            <Button
              primary
              onClick={() => action.mutate(() => serviceStart("php-fcgi"))}
              disabled={busy || installed.length === 0}
              icon={<Play />}
            >
              {t("actions.start")}
            </Button>
          )}
          <Button onClick={() => action.mutate(() => serviceOpenLog("php-fcgi"))} disabled={busy} icon={<FileText />}>
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
        <div className="mt-4 max-w-sm">
          <VersionSelect
            label={t("php.version")}
            newestLabel={t("php.newest", { version: installed[0]?.version })}
            versions={installed.map((v) => v.version)}
            value={config?.version ?? ""}
            disabled={busy || !config}
            onChange={(v) => action.mutate(() => phpSetVersion(v))}
          />
        </div>
      )}

      {rt && (
        <dl className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5">
          <Fact label={t("php.version")} value={rt.version} />
          <Fact
            label={t("php.port")}
            value={
              <span className="flex items-center gap-2">
                {rt.port}
                {rt.fellBack && (
                  <span className="rounded bg-warn-soft px-1.5 py-0.5 text-xs text-warn">{t("php.fallback")}</span>
                )}
              </span>
            }
          />
          <Fact label={t("php.processes")} value={status.processes} />
          <Fact label={t("php.children")} value={rt.children} />
          <Fact label={t("php.maxRequests")} value={rt.maxRequests === 0 ? t("php.unlimited") : rt.maxRequests} />
        </dl>
      )}

      {rt?.fellBack && (
        <p className="mt-3 text-sm text-warn">
          {t("php.fallbackHint", { configured: config?.fcgi_port, port: rt.port })}
        </p>
      )}

      <div className="mt-4 space-y-2">
        <ErrorBox error={action.error} />
        {!action.error && status.status === "failed" && status.error && (
          <ErrorBox error={{ code: "service", message: status.error }} />
        )}
      </div>
    </section>
  );
}
