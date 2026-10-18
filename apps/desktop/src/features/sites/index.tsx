import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Activity, Bug, Code, Drama, ExternalLink, FolderOpen, Pencil, RefreshCw, ShieldCheck, SquareTerminal } from "lucide-react";
import { type FormEvent, useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui";
import { editorOpen, terminalOpen } from "@/features/env-path/api";
import { openWww, PHP_VERSIONS_KEY, phpVersions } from "@/features/services/api";
import { DevServer, fullOverride, isDefaultOverride, ServedByFields } from "@/features/sites/DevServer";
import { HttpsCard } from "@/features/sites/HttpsCard";
import { Requests } from "@/features/sites/Requests";
import { WebhooksCard } from "@/features/sites/Webhooks";
import { formatDate, SSL_KEY, sslStatus } from "@/features/sites/ssl";
import { type AppConfig, SETTINGS_KEY, type SiteOverride, settingsGet, settingsUpdate } from "@/features/settings/api";
import { ShareButton, SharePanel, useSiteShare } from "@/features/tunnel";
import { qrSource, TUNNEL_KEY, type TunnelOverview, tunnelOverview } from "@/features/tunnel/api";
import {
  APPS_KEY,
  type AppStatus,
  appsStatus,
  LAN_KEY,
  type LanOverview,
  lanOverview,
  type HostsReport,
  type Site,
  SITES_KEY,
  type Skipped,
  siteOpen,
  siteOpenFolder,
  sitePlaywrightInstall,
  siteXdebugConfig,
  sitesList,
  sitesSync,
  type SitesOverview,
} from "@/features/sites/api";

/** Every project folder as `<folder>.<tld>`, plus the hosts file state. */
export function SitesPage() {
  const { t } = useTranslation();
  const sites = useQuery({ queryKey: SITES_KEY, queryFn: sitesList });
  const settings = useQuery({ queryKey: SETTINGS_KEY, queryFn: settingsGet });
  const action = useSitesAction();
  const data = sites.data;
  const config = settings.data?.config;

  return (
    <>
      <PageHeader
        title={t("nav.sites")}
        actions={
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => action.mutate(sitesSync)} disabled={action.isPending} icon={<RefreshCw />}>
              {t("sites.syncNow")}
            </Button>
            <Button onClick={() => action.mutate(openWww)} icon={<FolderOpen />}>
              {t("actions.openWww")}
            </Button>
          </div>
        }
      />
      <div className="space-y-4 px-8 pb-8">
        <ErrorBox error={sites.error ?? action.error} />
        {data && config && (
          <>
            <AutoSettings key={`${data.tld}:${data.autoVhosts}`} data={data} config={config} />
            <HttpsCard />
            <HostsStatus hosts={data.hosts} webRunning={data.webRunning} onRetry={() => action.mutate(sitesSync)} busy={action.isPending} />
            {data.autoVhosts && <SiteList data={data} config={config} />}
            <WebhooksCard config={config} sites={data.sites} />
          </>
        )}
      </div>
    </>
  );
}

/** Runs a sites action and refreshes the page. */
function useSitesAction() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: SITES_KEY }),
        queryClient.invalidateQueries({ queryKey: SETTINGS_KEY }),
      ]),
  });
}

/** "Automatic sites" switch and the domain suffix. */
function AutoSettings({ data, config }: { data: SitesOverview; config: AppConfig }) {
  const { t } = useTranslation();
  const [tld, setTld] = useState(data.tld);
  const save = useSitesAction();
  const update = (patch: Partial<AppConfig["general"]>) =>
    save.mutate(() => settingsUpdate("general", { ...config.general, ...patch }));
  const submit = (e: FormEvent) => {
    e.preventDefault();
    update({ tld: tld.trim() });
  };
  const cleaned = tld.trim().replace(/^\.+/, "").toLowerCase();

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="auto-title">
      <h2 id="auto-title" className="text-lg font-semibold">
        {t("sites.auto.title")}
      </h2>
      <p className="mt-1 text-sm text-muted">
        {t("sites.auto.description", { folder: data.wwwRoot, tld: data.tld, interpolation: { escapeValue: false } })}
      </p>
      <div className="mt-4 flex flex-wrap items-end gap-6">
        <label className="flex items-center gap-2.5 text-sm">
          <input
            type="checkbox"
            className="size-4 accent-accent"
            checked={data.autoVhosts}
            disabled={save.isPending}
            onChange={(e) => update({ auto_vhosts: e.target.checked })}
          />
          {t("sites.auto.enabled")}
        </label>
        <label className="flex items-center gap-2.5 text-sm" title={t("sites.lan.hint")}>
          <input
            type="checkbox"
            className="size-4 accent-accent"
            checked={config.general.lan}
            disabled={save.isPending}
            onChange={(e) => update({ lan: e.target.checked })}
          />
          {t("sites.lan.enabled")}
        </label>
        <form onSubmit={submit} className="flex items-end gap-2">
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="font-medium">{t("sites.auto.tld")}</span>
            <span className="flex items-center gap-1 font-mono" dir="ltr">
              .
              <input
                aria-label={t("sites.auto.tld")}
                className="w-28 rounded-md border border-line bg-surface px-3 py-1.5"
                value={tld}
                onChange={(e) => setTld(e.target.value)}
              />
            </span>
          </label>
          <button
            type="submit"
            disabled={save.isPending || cleaned === data.tld || cleaned === ""}
            className="rounded-md border border-line px-3 py-1.5 text-sm hover:bg-hover disabled:opacity-50"
          >
            {t("actions.save")}
          </button>
        </form>
      </div>
      {cleaned === "local" && <p className="mt-3 text-sm text-warn">{t("sites.auto.localWarning")}</p>}
      {config.general.lan && <p className="mt-3 text-sm text-muted">{t("sites.lan.hint")}</p>}
      <div className="mt-3">
        <ErrorBox error={save.error} />
      </div>
    </section>
  );
}

/** Whether the hosts file has every site, and what to do if not. */
export function HostsStatus(props: { hosts: HostsReport; webRunning: boolean; busy: boolean; onRetry: () => void }) {
  const { t } = useTranslation();
  const { hosts } = props;
  const missing = hosts.missing.length + hosts.extra.length;
  const outcome = hosts.last?.outcome;

  let tone: "ok" | "info" | "warn" | "danger" = "ok";
  let text: string;
  let retry = false;
  if (hosts.error) {
    tone = "danger";
    text = t("sites.hosts.unreadable", { error: hosts.error, interpolation: { escapeValue: false } });
  } else if (hosts.pending) {
    tone = "info";
    text = t("sites.hosts.pending");
  } else if (hosts.inSync) {
    text = t("sites.hosts.inSync");
  } else if (outcome === "declined") {
    tone = "warn";
    text = t("sites.hosts.declined", { count: missing });
    retry = true;
  } else if (outcome === "helperMissing") {
    tone = "danger";
    text = t("sites.hosts.helperMissing", { detail: hosts.last?.message ?? "", interpolation: { escapeValue: false } });
  } else if (outcome === "failed") {
    tone = "danger";
    text = t("sites.hosts.failed", { detail: hosts.last?.message ?? "", interpolation: { escapeValue: false } });
    retry = true;
  } else if (!props.webRunning) {
    tone = "info";
    text = t("sites.hosts.onStart", { count: missing });
    retry = true;
  } else {
    tone = "warn";
    text = t("sites.hosts.outOfDate", { count: missing });
    retry = true;
  }

  const colours = {
    ok: "border-line text-muted",
    info: "border-line",
    warn: "border-warn/40 bg-warn-soft",
    danger: "border-danger/40 bg-danger-soft text-danger",
  }[tone];

  return (
    <div role="status" className={`flex flex-wrap items-center justify-between gap-3 rounded-md border px-4 py-3 text-sm ${colours}`}>
      <div className="min-w-0">
        <p>{text}</p>
        <p className="mt-0.5 truncate font-mono text-xs opacity-70" dir="ltr">
          {hosts.file}
        </p>
      </div>
      {retry && !hosts.pending && (
        <Button onClick={props.onRetry} disabled={props.busy} icon={<ShieldCheck />}>
          {t("sites.hosts.update")}
        </Button>
      )}
    </div>
  );
}

function SiteList({ data, config }: { data: SitesOverview; config: AppConfig }) {
  const { t } = useTranslation();
  const tunnels = useQuery({ queryKey: TUNNEL_KEY, queryFn: tunnelOverview });
  const apps = useQuery({ queryKey: APPS_KEY, queryFn: appsStatus, enabled: data.sites.some((s) => s.app) });
  const lan = useQuery({ queryKey: LAN_KEY, queryFn: lanOverview, enabled: config.general.lan });
  const [search, setSearch] = useState("");
  const needle = search.trim().toLowerCase();
  const shown = needle ? data.sites.filter((s) => s.domain.includes(needle) || s.name.toLowerCase().includes(needle)) : data.sites;
  if (data.sites.length === 0 && data.skipped.length === 0) {
    return (
      <p className="rounded-xl border border-dashed border-line p-6 text-sm text-muted">
        {t("sites.empty", { folder: data.wwwRoot, tld: data.tld, interpolation: { escapeValue: false } })}
      </p>
    );
  }
  return (
    <>
      {!data.webRunning && data.sites.length > 0 && <p className="text-sm text-muted">{t("sites.startWeb")}</p>}
      {data.sites.length > 3 && (
        <input
          type="search"
          aria-label={t("sites.search")}
          placeholder={t("sites.search")}
          className="w-full max-w-sm rounded-md border border-line bg-surface px-3 py-1.5 text-sm"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      )}
      {shown.length === 0 && data.sites.length > 0 && <p className="text-sm text-muted">{t("sites.noMatch")}</p>}
      <ul className="divide-y divide-line rounded-xl border border-line empty:hidden" aria-label={t("nav.sites")}>
        {shown.map((site) => (
          <SiteRow
            key={site.name}
            site={site}
            config={config}
            webRunning={data.webRunning}
            tunnels={tunnels.data}
            app={apps.data?.find((a) => a.name === site.name)}
            lan={lan.data}
          />
        ))}
      </ul>
      {data.skipped.length > 0 && <SkippedList skipped={data.skipped} config={config} />}
    </>
  );
}

/** "public/", "(project folder)" or the configured path. */
function docrootLabel(t: (k: string) => string, site: Site): string {
  if (site.docrootSource.kind === "configured") return site.docroot;
  return site.docrootSource.subdir ? `${site.docrootSource.subdir}/` : t("sites.docrootFolder");
}

function SiteRow(props: {
  site: Site;
  config: AppConfig;
  webRunning: boolean;
  tunnels: TunnelOverview | undefined;
  app: AppStatus | undefined;
  lan: LanOverview | undefined;
}) {
  const { site, config, webRunning } = props;
  const { t, i18n } = useTranslation();
  const share = useSiteShare(site.name, props.tunnels, config.web);
  const ssl = useQuery({ queryKey: SSL_KEY, queryFn: sslStatus });
  const cert = ssl.data?.enabled ? ssl.data.sites.find((c) => c.domain === site.domain) : undefined;
  const [editing, setEditing] = useState(false);
  const [showRequests, setShowRequests] = useState(false);
  const action = useSitesAction();
  const current = fullOverride(config.sites[site.name]);
  const php = useQuery({ queryKey: PHP_VERSIONS_KEY, queryFn: phpVersions, enabled: editing });
  const [form, setForm] = useState<SiteOverride>(current);
  const servedByApp = form.kind === "app" || (form.kind === "auto" && (!!(site.app && !site.app.configured) || form.command.trim() !== ""));

  const saveOverride = (next: SiteOverride) =>
    action.mutate(async () => {
      const sites = { ...config.sites };
      const cleaned = { ...next, docroot: next.docroot.trim(), command: next.command.trim() };
      if (isDefaultOverride(cleaned)) delete sites[site.name];
      else sites[site.name] = cleaned;
      await settingsUpdate("sites", sites);
      setEditing(false);
    });

  return (
    <li className="px-4 py-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="font-mono text-sm font-medium" dir="ltr">
            {site.domain}
          </p>
          <p className="mt-0.5 truncate text-xs text-muted">
            {site.app
              ? t("sites.rowApp", { folder: site.name, framework: site.app.framework, interpolation: { escapeValue: false } })
              : t("sites.row", { folder: site.name, docroot: docrootLabel(t, site), interpolation: { escapeValue: false } })}
            {!site.app && site.php && ` · ${t("sites.phpVersion", { version: site.php })}`}
          </p>
          {cert && (
            <p className="mt-0.5 text-xs text-muted">
              {t("ssl.siteCert", { date: formatDate(cert.notAfter, i18n.language) })}
            </p>
          )}
          {!site.app && !site.docrootExists && <p className="mt-0.5 text-xs text-warn">{t("sites.docrootMissing")}</p>}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => action.mutate(() => siteOpen(site.domain))} disabled={!webRunning} icon={<ExternalLink />}>
            {t("sites.open")}
          </Button>
          <Button onClick={() => action.mutate(() => editorOpen({ kind: "site", name: site.name }))} icon={<Code />}>
            {t("editor.open")}
          </Button>
          <Button onClick={() => action.mutate(() => terminalOpen(site.path))} icon={<SquareTerminal />} aria-label={t("path.openTerminal")} title={t("path.openTerminal")} />
          <Button onClick={() => action.mutate(() => siteOpenFolder(site.name))} icon={<FolderOpen />}>
            {t("sites.folder")}
          </Button>
          <ShareButton share={share} disabled={!site.app && !site.docrootExists} />
          {site.node && (
            <Button onClick={() => action.mutate(() => sitePlaywrightInstall(site.name))} disabled={action.isPending} icon={<Drama />} title={t("sites.playwrightHint")}>
              {action.isPending ? t("sites.playwrightBusy") : t("sites.playwright")}
            </Button>
          )}
          {config.php.xdebug && !site.app && (
            <Button
              onClick={() =>
                action.mutate(async () => {
                  await siteXdebugConfig(site.name);
                  await editorOpen({ kind: "site", name: site.name });
                })
              }
              icon={<Bug />}
              title={t("sites.xdebugConfigHint")}
            >
              {t("sites.xdebugConfig")}
            </Button>
          )}
          <Button onClick={() => setShowRequests(!showRequests)} aria-expanded={showRequests} icon={<Activity />}>
            {t("sites.requests.button")}
          </Button>
          <Button onClick={() => setEditing(!editing)} aria-expanded={editing} icon={<Pencil />}>
            {t("sites.edit")}
          </Button>
        </div>
      </div>
      <LanAddress site={site} lan={props.lan} />
      <DevServer site={site} status={props.app} />
      {showRequests && <Requests name={site.name} />}
      <SharePanel share={share} />
      {editing && (
        <form
          className="mt-3 flex flex-wrap items-end gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            saveOverride({ ...form, enabled: true });
          }}
        >
          <ServedByFields site={site} value={form} onChange={setForm} />
          {!servedByApp && (
            <label className="flex flex-col gap-1.5 text-sm">
              <span className="font-medium">{t("sites.php")}</span>
              <select className="rounded-md border border-line bg-surface px-3 py-1.5" value={form.php} onChange={(e) => setForm({ ...form, php: e.target.value })}>
                <option value="">{t("sites.phpShared", { version: config.php.version || t("sites.phpNewest") })}</option>
                {(php.data ?? []).map((v) => (
                  <option key={v.version} value={v.version}>
                    {t("sites.phpVersion", { version: v.version })}
                  </option>
                ))}
                {form.php && !(php.data ?? []).some((v) => v.version === form.php) && <option value={form.php}>{t("sites.phpVersion", { version: form.php })}</option>}
              </select>
            </label>
          )}
          {!servedByApp && (
            <label className="flex min-w-64 flex-1 flex-col gap-1.5 text-sm">
              <span className="font-medium">{t("sites.docroot")}</span>
              <input
                className="rounded-md border border-line bg-surface px-3 py-1.5 font-mono"
                dir="ltr"
                placeholder={t("sites.docrootPlaceholder")}
                value={form.docroot}
                onChange={(e) => setForm({ ...form, docroot: e.target.value })}
              />
            </label>
          )}
          <button type="submit" disabled={action.isPending} className="rounded-md bg-accent px-3 py-1.5 text-sm text-on-accent hover:bg-accent-strong disabled:opacity-50">
            {t("actions.save")}
          </button>
          <button
            type="button"
            disabled={action.isPending}
            onClick={() => saveOverride({ ...current, enabled: false })}
            className="rounded-md border border-line px-3 py-1.5 text-sm hover:bg-hover disabled:opacity-50"
          >
            {t("sites.disable")}
          </button>
        </form>
      )}
      <div className="mt-2">
        <ErrorBox error={action.error} />
      </div>
    </li>
  );
}

/** The site's address for phones and tablets on the same network, with a QR code. */
function LanAddress({ site, lan }: { site: Site; lan: LanOverview | undefined }) {
  const { t } = useTranslation();
  const [qr, setQr] = useState(false);
  const entry = lan?.sites.find((s) => s.name === site.name);
  if (!lan?.enabled) return null;
  if (!entry) return <p className="mt-1 text-xs text-warn">{t("sites.lan.noNetwork")}</p>;
  return (
    <div className="mt-1 text-xs text-muted">
      {t("sites.lan.address")}{" "}
      <button type="button" className="font-mono text-accent-ink underline" dir="ltr" onClick={() => setQr(!qr)} aria-expanded={qr}>
        {entry.url}
      </button>
      {qr && <img className="mt-2 size-40 rounded bg-white p-2" src={qrSource(entry.qrSvg)} alt={t("sites.lan.qr", { url: entry.url })} />}
    </div>
  );
}

function SkippedList({ skipped, config }: { skipped: Skipped[]; config: AppConfig }) {
  const { t } = useTranslation();
  const action = useSitesAction();
  const enable = (name: string) =>
    action.mutate(() => {
      const sites = { ...config.sites };
      const next = { ...fullOverride(sites[name]), enabled: true };
      if (isDefaultOverride(next)) delete sites[name];
      else sites[name] = next;
      return settingsUpdate("sites", sites);
    });

  const reason = (s: Skipped) => {
    switch (s.reason.kind) {
      case "disabled":
        return t("sites.skipped.disabled");
      case "invalidName":
        return t("sites.skipped.invalidName");
      case "unsafePath":
        return t("sites.skipped.unsafePath");
      case "duplicateDomain":
        return t("sites.skipped.duplicate", { domain: s.reason.domain, other: s.reason.takenBy, interpolation: { escapeValue: false } });
    }
  };

  return (
    <section aria-labelledby="skipped-title">
      <h2 id="skipped-title" className="mb-2 mt-2 text-sm font-semibold text-muted">
        {t("sites.skipped.title")}
      </h2>
      <ul className="divide-y divide-line rounded-xl border border-line">
        {skipped.map((s) => (
          <li key={s.name} className="flex flex-wrap items-center justify-between gap-3 px-4 py-2.5 text-sm">
            <span className="min-w-0">
              <span className="font-mono">{s.name}</span> <span className="text-muted">— {reason(s)}</span>
            </span>
            {s.reason.kind === "disabled" && (
              <Button onClick={() => enable(s.name)} disabled={action.isPending}>
                {t("sites.enable")}
              </Button>
            )}
          </li>
        ))}
      </ul>
      <div className="mt-2">
        <ErrorBox error={action.error} />
      </div>
    </section>
  );
}
