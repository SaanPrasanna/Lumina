import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Play, RotateCcw, Square, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { Button } from "@/components/ui";
import { type AppConfig, SETTINGS_KEY, settingsUpdate } from "@/features/settings/api";
import { type Hook, type Site, WEBHOOKS_KEY, webhookReplay, webhooksClear, webhooksStart, webhooksStatus, webhooksStop } from "@/features/sites/api";

/** The webhook catcher: a local endpoint that records requests and can pass them on to a site. */
export function WebhooksCard({ config, sites }: { config: AppConfig; sites: Site[] }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const status = useQuery({ queryKey: WEBHOOKS_KEY, queryFn: webhooksStatus, refetchInterval: (q) => (q.state.data?.port ? 2000 : false) });
  const action = useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () =>
      Promise.all([queryClient.invalidateQueries({ queryKey: WEBHOOKS_KEY }), queryClient.invalidateQueries({ queryKey: SETTINGS_KEY })]),
  });
  const [port, setPort] = useState(config.webhooks.port);
  const listening = status.data?.port ?? null;
  const busy = action.isPending;
  const hooks = status.data?.hooks ?? [];

  const save = (patch: Partial<AppConfig["webhooks"]>) =>
    action.mutate(async () => {
      await settingsUpdate("webhooks", { ...config.webhooks, ...patch });
      if (listening) await webhooksStart();
    });

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="webhooks-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 id="webhooks-title" className="text-lg font-semibold">
            {t("webhooks.title")}
          </h2>
          <p className="mt-1 text-sm text-muted">{t("webhooks.description")}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          {listening ? (
            <Button onClick={() => action.mutate(webhooksStop)} disabled={busy} icon={<Square />}>
              {t("actions.stop")}
            </Button>
          ) : (
            <Button primary onClick={() => action.mutate(webhooksStart)} disabled={busy} icon={<Play />}>
              {t("actions.start")}
            </Button>
          )}
          <Button onClick={() => action.mutate(webhooksClear)} disabled={busy || hooks.length === 0} icon={<Trash2 />}>
            {t("webhooks.clear")}
          </Button>
        </div>
      </div>

      <div className="mt-4 flex flex-wrap items-end gap-4">
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("webhooks.port")}</span>
          <input
            type="number"
            min={1}
            max={65535}
            dir="ltr"
            className="w-28 rounded-md border border-line bg-surface px-3 py-1.5 font-mono"
            value={port}
            disabled={busy}
            onChange={(e) => setPort(Number(e.target.value))}
            onBlur={() => port > 0 && port !== config.webhooks.port && save({ port })}
          />
        </label>
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("webhooks.forward")}</span>
          <select className="rounded-md border border-line bg-surface px-3 py-1.5" value={config.webhooks.forward} disabled={busy} onChange={(e) => save({ forward: e.target.value })}>
            <option value="">{t("webhooks.recordOnly")}</option>
            {sites.map((s) => (
              <option key={s.name} value={s.name}>
                {s.domain}
              </option>
            ))}
          </select>
        </label>
        {listening && (
          <p className="pb-1.5 font-mono text-sm" dir="ltr">
            {t("webhooks.url", { port: listening })}
          </p>
        )}
      </div>

      <div className="mt-3">
        <ErrorBox error={action.error ?? status.error} />
      </div>
      {hooks.length > 0 && (
        <ul className="mt-3 divide-y divide-line rounded-md border border-line">
          {hooks.map((hook) => (
            <HookRow key={hook.id} hook={hook} sites={sites} defaultSite={config.webhooks.forward} onReplay={(site) => action.mutate(() => webhookReplay(hook.id, site))} busy={busy} />
          ))}
        </ul>
      )}
    </section>
  );
}

function HookRow(props: { hook: Hook; sites: Site[]; defaultSite: string; onReplay: (site: string) => void; busy: boolean }) {
  const { hook } = props;
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const [site, setSite] = useState(props.defaultSite || props.sites[0]?.name || "");
  const pretty = (() => {
    try {
      return JSON.stringify(JSON.parse(hook.body), null, 2);
    } catch {
      return hook.body;
    }
  })();

  return (
    <li className="px-3 py-2 text-sm">
      <button type="button" className="flex w-full flex-wrap items-center gap-x-3 gap-y-1 text-start" aria-expanded={open} onClick={() => setOpen(!open)}>
        <span className="text-muted">{new Date(hook.receivedAt).toLocaleTimeString(i18n.language)}</span>
        <span className="font-mono" dir="ltr">
          {t("sites.requests.line", { method: hook.method, uri: hook.path })}
        </span>
        <span className="text-muted">{t("webhooks.size", { size: hook.size })}</span>
        {hook.forwarded && <span className="text-muted">{t("webhooks.forwarded", { result: hook.forwarded })}</span>}
      </button>
      {open && (
        <div className="mt-2 space-y-2">
          <pre className="max-h-40 overflow-auto rounded bg-hover p-2 font-mono text-xs" dir="ltr">
            {hook.headers.map(([k, v]) => `${k}: ${v}`).join("\n")}
          </pre>
          <pre className="max-h-72 overflow-auto rounded bg-hover p-2 font-mono text-xs" dir="ltr">
            {pretty || t("webhooks.noBody")}
          </pre>
          {hook.truncated && <p className="text-xs text-warn">{t("webhooks.truncated")}</p>}
          {props.sites.length > 0 && (
            <div className="flex flex-wrap items-center gap-2">
              <select className="rounded-md border border-line bg-surface px-2 py-1 text-sm" value={site} onChange={(e) => setSite(e.target.value)} aria-label={t("webhooks.replayTo")}>
                {props.sites.map((s) => (
                  <option key={s.name} value={s.name}>
                    {s.domain}
                  </option>
                ))}
              </select>
              <Button onClick={() => props.onReplay(site)} disabled={props.busy || !site} icon={<RotateCcw />}>
                {t("webhooks.replay")}
              </Button>
            </div>
          )}
        </div>
      )}
    </li>
  );
}
