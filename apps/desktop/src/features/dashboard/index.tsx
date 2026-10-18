import { useQuery } from "@tanstack/react-query";
import { FolderOpen, Globe, Play, Rocket, Share2, Square } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { InstallButton, NotInstalledBadge } from "@/components/Installable";
import { StatusBadge } from "@/components/StatusBadge";
import { packageOf, useInstalled } from "@/features/services/installed";
import { Button } from "@/components/ui";
import { BOOTSTRAP_QUERY_KEY, bootstrapStatus } from "@/features/onboarding/api";
import {
  dbKind,
  openLocalhost,
  openWww,
  SERVICES_KEY,
  serviceStart,
  serviceStop,
  servicesStatus,
  servicesStopAll,
  type StackStatus,
  webKind,
} from "@/features/services/api";
import { PortConflictDialog } from "@/features/services/PortConflictDialog";
import { useStartAll } from "@/features/services/useStartAll";
import { SETTINGS_KEY, settingsGet } from "@/features/settings/api";
import { SITES_KEY, sitesList } from "@/features/sites/api";
import { TUNNEL_KEY, tunnelOverview } from "@/features/tunnel/api";
import { isCommandError } from "@/lib/ipc";
import type { ServiceKind, ServiceStatus } from "@/types";

/** Everything at a glance: the services, quick actions, sites and shares. */
export function DashboardPage() {
  const { t } = useTranslation();
  const bootstrap = useQuery({ queryKey: BOOTSTRAP_QUERY_KEY, queryFn: bootstrapStatus });
  const settings = useQuery({ queryKey: SETTINGS_KEY, queryFn: settingsGet });
  const stack = useQuery({ queryKey: SERVICES_KEY, queryFn: servicesStatus });
  const sites = useQuery({ queryKey: SITES_KEY, queryFn: sitesList });
  const tunnels = useQuery({ queryKey: TUNNEL_KEY, queryFn: tunnelOverview });
  const { action, startAll, conflict, setConflict } = useStartAll(settings.data?.config);
  const data = stack.data;
  const anyRunning = !!(data?.php.runtime || data?.web.runtime || data?.database.runtime || data?.redis.runtime || data?.postgresql.runtime || data?.mailpit.runtime);

  return (
    <>
      <PageHeader
        title={t("nav.dashboard")}
        actions={
          <div className="flex flex-wrap gap-2">
            {anyRunning ? (
              <Button onClick={() => action.mutate(servicesStopAll)} disabled={action.isPending} icon={<Square />}>
                {t("actions.stopAll")}
              </Button>
            ) : (
              <Button primary onClick={startAll} disabled={action.isPending} icon={<Play />}>
                {t("actions.startAll")}
              </Button>
            )}
            <Button onClick={() => action.mutate(openLocalhost)} disabled={!data?.web.runtime} icon={<Globe />}>
              {t("actions.openLocalhost")}
            </Button>
            <Button onClick={() => action.mutate(openWww)} icon={<FolderOpen />}>
              {t("actions.openWww")}
            </Button>
          </div>
        }
      />
      <div className="space-y-6 px-8 pb-8">
        {!(isCommandError(action.error) && action.error.code === "port_in_use") && <ErrorBox error={action.error ?? stack.error} />}
        {data && <ServiceGrid data={data} onAction={(fn) => action.mutate(fn)} busy={action.isPending} />}

        <div className="grid gap-4 sm:grid-cols-3">
          <Tile
            to="/sites"
            title={t("dashboard.sites")}
            value={sites.data ? String(sites.data.sites.length) : "–"}
            hint={sites.data ? t("dashboard.sitesHint", { tld: sites.data.tld }) : ""}
            icon={<Globe />}
          />
          <Tile
            to="/tunnel"
            title={t("dashboard.shared")}
            value={tunnels.data ? String(tunnels.data.tunnels.length) : "–"}
            hint={t("dashboard.sharedHint")}
            icon={<Share2 />}
          />
          <Tile to="/quickapp" title={t("dashboard.newProject")} value="" hint={t("dashboard.newProjectHint")} icon={<Rocket />} />
        </div>

        {bootstrap.data?.dataRoot && (
          <div className="flex items-center gap-3 rounded-lg border border-line p-4">
            <FolderOpen className="size-5 shrink-0 text-accent-ink" aria-hidden />
            <div className="min-w-0">
              <div className="text-xs text-muted">{t("dashboard.dataRoot")}</div>
              <div className="truncate font-mono text-sm" dir="ltr">
                {bootstrap.data.dataRoot}
              </div>
            </div>
          </div>
        )}
      </div>
      {conflict && <PortConflictDialog request={conflict} onClose={() => setConflict(null)} />}
    </>
  );
}

interface Row {
  kind: ServiceKind;
  name: string;
  status: ServiceStatus;
  version: string | null;
  port: number | null;
  error: string | null;
}

function rows(data: StackStatus): Row[] {
  const web = data.web.server === "nginx" ? "nginx" : "Apache";
  const db = data.database.engine === "mysql" ? "MySQL" : "MariaDB";
  return [
    { kind: "php-fcgi", name: "PHP", status: data.php.status, version: data.php.runtime?.version ?? null, port: data.php.runtime?.port ?? null, error: data.php.error },
    { kind: webKind(data.web.server), name: web, status: data.web.status, version: data.web.runtime?.version ?? null, port: data.web.runtime?.httpPort ?? null, error: data.web.error },
    { kind: dbKind(data.database.engine), name: db, status: data.database.status, version: data.database.runtime?.version ?? null, port: data.database.runtime?.port ?? null, error: data.database.error },
    { kind: "postgresql", name: "PostgreSQL", status: data.postgresql.status, version: data.postgresql.runtime?.version ?? null, port: data.postgresql.runtime?.port ?? null, error: data.postgresql.error },
    { kind: "redis", name: "Redis", status: data.redis.status, version: data.redis.runtime?.version ?? null, port: data.redis.runtime?.port ?? null, error: data.redis.error },
    { kind: "mailpit", name: "Mailpit", status: data.mailpit.status, version: data.mailpit.runtime?.version ?? null, port: data.mailpit.runtime?.smtpPort ?? null, error: data.mailpit.error },
  ];
}

function ServiceGrid({ data, onAction, busy }: { data: StackStatus; onAction: (fn: () => Promise<unknown>) => void; busy: boolean }) {
  const { t } = useTranslation();
  return (
    <section aria-labelledby="dash-services">
      <div className="mb-2 flex items-center justify-between">
        <h2 id="dash-services" className="text-sm font-semibold text-muted">
          {t("nav.services")}
        </h2>
        <Link to="/services" className="rounded text-sm text-accent-ink outline-none hover:underline focus-visible:ring-2 focus-visible:ring-accent">
          {t("dashboard.manage")}
        </Link>
      </div>
      <ul className="grid gap-3 sm:grid-cols-2">
        {rows(data).map((row) => (
          <ServiceTile key={row.kind} row={row} onAction={onAction} busy={busy} />
        ))}
      </ul>
    </section>
  );
}

function Tile({ to, title, value, hint, icon }: { to: string; title: string; value: string; hint: string; icon: ReactNode }) {
  return (
    <Link
      to={to}
      className="flex items-start gap-3 rounded-lg border border-line p-4 outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-accent [&_svg]:size-5"
    >
      <span className="mt-0.5 shrink-0 text-accent-ink" aria-hidden>
        {icon}
      </span>
      <span className="min-w-0">
        <span className="block text-sm font-medium">{title}</span>
        {value && <span className="block text-2xl font-semibold">{value}</span>}
        <span className="block truncate text-xs text-muted">{hint}</span>
      </span>
    </Link>
  );
}

function ServiceTile({ row, onAction, busy }: { row: Row; onAction: (fn: () => Promise<unknown>) => void; busy: boolean }) {
  const { t } = useTranslation();
  const running = row.status === "running";
  const missing = useInstalled(row.kind) === false && !running;
  return (
    <li className="flex items-center justify-between gap-3 rounded-lg border border-line p-4">
      <div className="min-w-0">
        <p className="font-medium">
          {row.name}
          {row.version && <span className="ms-2 font-mono text-xs text-muted">{row.version}</span>}
        </p>
        <div className="mt-1 flex flex-wrap items-center gap-3">
          {missing ? <NotInstalledBadge /> : <StatusBadge status={row.status} />}
          {row.port != null && <span className="font-mono text-xs text-muted">{t("dashboard.port", { port: row.port })}</span>}
        </div>
        {row.error && <p className="mt-1 truncate text-xs text-danger">{row.error}</p>}
      </div>
      {missing ? (
        <InstallButton pkg={packageOf(row.kind)} name={row.name} primary={false} />
      ) : (
        <Button
          onClick={() => onAction(() => (running ? serviceStop(row.kind) : serviceStart(row.kind)))}
          disabled={busy || row.status === "starting" || row.status === "stopping"}
          aria-label={running ? t("dashboard.stopService", { name: row.name }) : t("dashboard.startService", { name: row.name })}
          icon={running ? <Square /> : <Play />}
        >
          {running ? t("actions.stop") : t("actions.start")}
        </Button>
      )}
    </li>
  );
}
