import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CheckCircle2, Download, FileText, KeyRound, Minus, Square, TriangleAlert } from "lucide-react";
import { type FormEvent, useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui";
import { PACKAGES_KEY, packageInstall, packagesList } from "@/features/packages/api";
import {
  DOMAINS_URL,
  SIGNUP_URL,
  TOKEN_URL,
  TUNNEL_KEY,
  type TunnelOverview,
  tunnelClearToken,
  tunnelOpenLog,
  tunnelOverview,
  tunnelSetDomain,
  tunnelSetToken,
  tunnelStopAll,
} from "@/features/tunnel/api";
import { TunnelCard } from "@/features/tunnel/TunnelCard";

export { ShareButton, SharePanel, useSiteShare } from "@/features/tunnel/SiteShare";

/** Sharing: share local sites on a public HTTPS address through ngrok. */
export function TunnelPage() {
  const { t } = useTranslation();
  const overview = useQuery({ queryKey: TUNNEL_KEY, queryFn: tunnelOverview });
  const action = useTunnelAction();
  const data = overview.data;

  return (
    <>
      <PageHeader
        title={t("nav.tunnel")}
        actions={
          <div className="flex flex-wrap gap-2">
            {data && data.tunnels.length > 1 && (
              <Button onClick={() => action.mutate(tunnelStopAll)} disabled={action.isPending} icon={<Square />}>
                {t("tunnel.stopAll")}
              </Button>
            )}
            <Button onClick={() => action.mutate(tunnelOpenLog)} icon={<FileText />}>
              {t("tunnel.openLog")}
            </Button>
          </div>
        }
      />
      <div className="space-y-4 px-8 pb-8">
        <p className="-mt-2 text-sm text-muted">{t("tunnel.intro")}</p>
        <ErrorBox error={overview.error ?? action.error} />
        {data && (
          <>
            {data.stopped && (
              <p role="alert" className="flex items-start gap-2 rounded-md border border-warn/40 bg-warn-soft px-4 py-3 text-sm">
                <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warn" aria-hidden />
                <span>
                  {t("tunnel.stopped")} <span className="font-mono text-xs" dir="ltr">{data.stopped}</span>
                </span>
              </p>
            )}
            <AgentCard data={data} />
            <TokenCard data={data} />
            <DomainCard key={data.domain} data={data} />
            <section aria-labelledby="shared-title">
              <h2 id="shared-title" className="mb-2 mt-2 text-sm font-semibold text-muted">
                {t("tunnel.sharedTitle")}
              </h2>
              {data.tunnels.length === 0 ? (
                <p className="rounded-xl border border-dashed border-line p-6 text-sm text-muted">{t("tunnel.none")}</p>
              ) : (
                <div className="space-y-3">
                  {data.tunnels.map((tunnel) => (
                    <TunnelCard key={tunnel.name} tunnel={tunnel} />
                  ))}
                </div>
              )}
            </section>
          </>
        )}
      </div>
    </>
  );
}

function useTunnelAction() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () => queryClient.invalidateQueries({ queryKey: TUNNEL_KEY }),
  });
}

function Step({ done, title, children }: { done: boolean; title: string; children: React.ReactNode }) {
  return (
    <section className="rounded-xl border border-line p-5" aria-label={title}>
      <div className="flex items-center gap-2.5">
        {done ? <CheckCircle2 className="size-5 text-ok" aria-hidden /> : <Minus className="size-5 text-muted" aria-hidden />}
        <h2 className="font-semibold">{title}</h2>
      </div>
      <div className="mt-3 space-y-3 text-sm">{children}</div>
    </section>
  );
}

/** 1. The ngrok agent, installed from Quick Add. */
function AgentCard({ data }: { data: TunnelOverview }) {
  const { t } = useTranslation();
  const packages = useQuery({ queryKey: PACKAGES_KEY, queryFn: packagesList, enabled: !data.installed });
  const install = useMutation({ mutationFn: (version: string) => packageInstall("ngrok", version) });
  const job = packages.data?.packages.find((p) => p.id === "ngrok")?.jobs[0];
  const installing = install.isPending || (job != null && !job.done);
  const percent = job?.total ? Math.round((job.received / job.total) * 100) : null;

  return (
    <Step done={data.installed != null} title={t("tunnel.agent.title")}>
      {data.installed ? (
        <p className="text-muted">{t("tunnel.agent.installed", { version: data.installed })}</p>
      ) : (
        <>
          <p className="text-muted">{t("tunnel.agent.missing")}</p>
          {data.available ? (
            <Button primary onClick={() => install.mutate(data.available ?? "")} disabled={installing} icon={<Download />}>
              {installing
                ? percent != null
                  ? t("tunnel.agent.installingPercent", { percent })
                  : t("tunnel.agent.installing")
                : t("tunnel.agent.install", { version: data.available })}
            </Button>
          ) : (
            <p className="text-warn">{t("tunnel.agent.unavailable")}</p>
          )}
          {job?.error && <p className="text-danger">{job.error}</p>}
          <ErrorBox error={install.error} />
        </>
      )}
    </Step>
  );
}

/** 2. The authtoken, kept in the system keychain. */
function TokenCard({ data }: { data: TunnelOverview }) {
  const { t } = useTranslation();
  const [token, setToken] = useState("");
  const [replacing, setReplacing] = useState(false);
  const action = useTunnelAction();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    action.mutate(async () => {
      await tunnelSetToken(token);
      setToken("");
      setReplacing(false);
    });
  };

  return (
    <Step done={data.hasToken} title={t("tunnel.token.title")}>
      {data.tokenError && <p className="text-danger">{t("tunnel.token.keychainError", { error: data.tokenError, interpolation: { escapeValue: false } })}</p>}
      {data.hasToken && !replacing ? (
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-muted">{t("tunnel.token.saved")}</p>
          <div className="flex gap-2">
            <Button onClick={() => setReplacing(true)} icon={<KeyRound />}>
              {t("tunnel.token.replace")}
            </Button>
            <Button onClick={() => action.mutate(tunnelClearToken)} disabled={action.isPending}>
              {t("tunnel.token.remove")}
            </Button>
          </div>
        </div>
      ) : (
        <>
          <p className="text-muted">
            {t("tunnel.token.help")}{" "}
            <button type="button" className="text-accent underline" onClick={() => void openUrl(SIGNUP_URL)}>
              {t("tunnel.token.signup")}
            </button>{" "}
            ·{" "}
            <button type="button" className="text-accent underline" onClick={() => void openUrl(TOKEN_URL)}>
              {t("tunnel.token.get")}
            </button>
          </p>
          <form onSubmit={submit} className="flex flex-wrap items-end gap-2">
            <label className="flex min-w-64 flex-1 flex-col gap-1.5">
              <span className="font-medium">{t("tunnel.token.label")}</span>
              <input
                type="password"
                autoComplete="off"
                spellCheck={false}
                className="rounded-md border border-line bg-surface px-3 py-1.5 font-mono"
                dir="ltr"
                value={token}
                onChange={(e) => setToken(e.target.value)}
              />
            </label>
            <button
              type="submit"
              disabled={action.isPending || token.trim() === ""}
              className="rounded-md bg-accent px-3 py-1.5 text-sm text-white hover:bg-accent-strong disabled:opacity-50"
            >
              {t("actions.save")}
            </button>
            {replacing && (
              <button type="button" onClick={() => setReplacing(false)} className="rounded-md border border-line px-3 py-1.5 text-sm hover:bg-hover">
                {t("actions.cancel")}
              </button>
            )}
          </form>
          <p className="text-xs text-muted">{t("tunnel.token.privacy")}</p>
        </>
      )}
      <ErrorBox error={action.error} />
    </Step>
  );
}

/** Optional: a reserved domain, so the address stays the same. */
function DomainCard({ data }: { data: TunnelOverview }) {
  const { t } = useTranslation();
  const [domain, setDomain] = useState(data.domain);
  const action = useTunnelAction();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    action.mutate(async () => setDomain(await tunnelSetDomain(domain)));
  };

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="domain-title">
      <h2 id="domain-title" className="font-semibold">
        {t("tunnel.domain.title")}
      </h2>
      <p className="mt-1 text-sm text-muted">
        {t("tunnel.domain.help")}{" "}
        <button type="button" className="text-accent underline" onClick={() => void openUrl(DOMAINS_URL)}>
          {t("tunnel.domain.find")}
        </button>
      </p>
      <form onSubmit={submit} className="mt-3 flex flex-wrap items-end gap-2 text-sm">
        <label className="flex min-w-64 flex-1 flex-col gap-1.5">
          <span className="font-medium">{t("tunnel.domain.label")}</span>
          <input
            className="rounded-md border border-line bg-surface px-3 py-1.5 font-mono"
            dir="ltr"
            placeholder="my-app.ngrok-free.app"
            spellCheck={false}
            value={domain}
            onChange={(e) => setDomain(e.target.value)}
          />
        </label>
        <button
          type="submit"
          disabled={action.isPending || domain.trim() === data.domain}
          className="rounded-md border border-line px-3 py-1.5 text-sm hover:bg-hover disabled:opacity-50"
        >
          {t("actions.save")}
        </button>
      </form>
      <div className="mt-3">
        <ErrorBox error={action.error} />
      </div>
    </section>
  );
}
