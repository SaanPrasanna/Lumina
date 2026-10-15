import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { Download, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { Button } from "@/components/ui";
import { formatBytes } from "@/features/packages/api";
import {
  type Channel,
  UPDATES_KEY,
  type UpdateProgress,
  type UpdateStatus,
  updatesCheck,
  updatesConfigure,
  updatesInstall,
  updatesStatus,
} from "@/features/updates/api";

/** Settings › About and updates: version, channel, check, install. */
export function UpdatesCard() {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const status = useQuery({ queryKey: UPDATES_KEY, queryFn: updatesStatus });
  const save = (next: UpdateStatus) => queryClient.setQueryData(UPDATES_KEY, next);
  const check = useMutation({ mutationFn: updatesCheck, onSuccess: save });
  const configure = useMutation({
    mutationFn: ({ channel, auto }: { channel: Channel; auto: boolean }) => updatesConfigure(channel, auto),
    onSuccess: save,
  });
  const install = useMutation({ mutationFn: updatesInstall, onSettled: () => queryClient.invalidateQueries({ queryKey: UPDATES_KEY }) });
  const [confirm, setConfirm] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);

  useEffect(() => {
    const unlisten = listen<UpdateProgress>("update-progress", (e) => setProgress(e.payload));
    return () => void unlisten.then((fn) => fn());
  }, []);

  const data = status.data;
  if (!data) return <ErrorBox error={status.error} />;
  const available = data.available;
  const installing = data.installing || install.isPending;
  const checkedAt = data.checkedAt ? new Date(data.checkedAt * 1000).toLocaleString(i18n.language) : null;

  return (
    <section id="updates" className="scroll-mt-4 space-y-4 rounded-xl border border-line p-5" aria-labelledby="updates-title">
      <h2 id="updates-title" className="text-lg font-semibold">
        {t("settings.updates.title")}
      </h2>
      <p className="text-sm">{t("settings.about.version", { version: data.current })}</p>

      <div className="flex flex-wrap items-end gap-4 text-sm">
        <label className="flex flex-col gap-1.5">
          <span className="font-medium">{t("settings.updates.channel")}</span>
          <select
            className="rounded-md border border-line bg-surface px-3 py-2"
            value={data.channel}
            disabled={configure.isPending || installing}
            onChange={(e) => configure.mutate({ channel: e.target.value as Channel, auto: data.check })}
          >
            <option value="stable">{t("settings.updates.stable")}</option>
            <option value="beta">{t("settings.updates.beta")}</option>
          </select>
        </label>
        <label className="flex items-center gap-2.5 pb-2">
          <input
            type="checkbox"
            className="size-4 accent-accent"
            checked={data.check}
            disabled={configure.isPending}
            onChange={(e) => configure.mutate({ channel: data.channel as Channel, auto: e.target.checked })}
          />
          {t("settings.updates.auto")}
        </label>
        <Button onClick={() => check.mutate()} disabled={check.isPending || installing} icon={<RefreshCw className={check.isPending ? "animate-spin" : ""} />}>
          {t("settings.updates.checkNow")}
        </Button>
      </div>

      <div role="status" className="text-sm">
        {available ? (
          <div className="space-y-3 rounded-md border border-accent/40 bg-accent-soft p-4">
            <p className="font-medium">{t("settings.updates.available", { version: available.version })}</p>
            {available.notes && <p className="whitespace-pre-wrap text-xs text-fg-soft">{available.notes}</p>}
            {installing ? (
              <p>
                {progress
                  ? t("settings.updates.downloading", {
                      received: formatBytes(progress.received),
                      total: progress.total ? formatBytes(progress.total) : "?",
                    })
                  : t("settings.updates.starting")}
              </p>
            ) : confirm ? (
              <div className="space-y-2">
                <p className="text-xs">{t("settings.updates.confirm")}</p>
                <div className="flex gap-2">
                  <Button primary onClick={() => install.mutate()} icon={<Download />}>
                    {t("settings.updates.installNow")}
                  </Button>
                  <Button onClick={() => setConfirm(false)}>{t("actions.cancel")}</Button>
                </div>
              </div>
            ) : (
              <Button primary onClick={() => setConfirm(true)} icon={<Download />}>
                {t("settings.updates.install")}
              </Button>
            )}
          </div>
        ) : (
          checkedAt && !data.error && <p className="text-muted">{t("settings.updates.upToDate", { when: checkedAt })}</p>
        )}
        {data.error && <p className="mt-2 text-xs text-danger">{t("settings.updates.failed", { error: data.error, interpolation: { escapeValue: false } })}</p>}
      </div>
      <p className="text-xs text-muted">{t("settings.updates.safety")}</p>
      <ErrorBox error={check.error ?? configure.error ?? install.error} />
    </section>
  );
}

/** A slim banner on every page while an update waits. */
export function UpdateBanner({ onOpen }: { onOpen: () => void }) {
  const { t } = useTranslation();
  const status = useQuery({ queryKey: UPDATES_KEY, queryFn: updatesStatus });
  const [dismissed, setDismissed] = useState<string | null>(null);
  const version = status.data?.available?.version;
  if (!version || dismissed === version || status.data?.installing) return null;
  return (
    <div role="status" className="flex flex-wrap items-center justify-between gap-3 border-b border-line bg-accent-soft px-8 py-2 text-sm">
      <span>{t("settings.updates.available", { version })}</span>
      <span className="flex gap-2">
        <Button primary onClick={onOpen} icon={<Download />}>
          {t("settings.updates.view")}
        </Button>
        <Button onClick={() => setDismissed(version)}>{t("settings.updates.later")}</Button>
      </span>
    </div>
  );
}
