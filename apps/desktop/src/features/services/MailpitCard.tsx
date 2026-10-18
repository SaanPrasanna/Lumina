import { useQuery } from "@tanstack/react-query";
import { FileText, Inbox, Play, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { InstallButton, NotInstalledBadge } from "@/components/Installable";
import { StatusBadge } from "@/components/StatusBadge";
import { Button, Fact, VersionSelect } from "@/components/ui";
import {
  MAILPIT_VERSIONS_KEY,
  type MailpitStatus,
  mailpitOpen,
  mailpitVersions,
  serviceOpenLog,
  serviceRestart,
  serviceStart,
  serviceStop,
} from "@/features/services/api";
import type { PortConflictRequest } from "@/features/services/PortConflictDialog";
import { useServiceAction } from "@/features/services/useServiceAction";
import { type MailpitSettings, settingsUpdate } from "@/features/settings/api";
import { isCommandError, type PortConflict, portConflictOf } from "@/lib/ipc";

/** Mailpit: catches outgoing mail; "include in Start All", version, inbox. */
export function MailpitCard(props: {
  status: MailpitStatus;
  config: MailpitSettings | undefined;
  onPortConflict: (request: PortConflictRequest) => void;
}) {
  const { t } = useTranslation();
  const { status, config } = props;
  const versions = useQuery({ queryKey: MAILPIT_VERSIONS_KEY, queryFn: mailpitVersions });
  const action = useServiceAction();

  const rt = status.runtime;
  const running = rt !== null;
  const busy = action.isPending;
  const installed = versions.data ?? [];
  const missing = versions.isSuccess && installed.length === 0;

  const run = (fn: () => Promise<unknown>) =>
    action.mutate(fn, {
      onError: (err) => {
        const request = config && mailpitConflictRequest(err, config);
        if (request) props.onPortConflict(request);
      },
    });

  const update = (patch: Partial<MailpitSettings>, restart = false) => {
    if (!config) return;
    run(async () => {
      await settingsUpdate("mailpit", { ...config, ...patch });
      if (restart && running) await serviceRestart("mailpit");
    });
  };

  const hideDialogError = isCommandError(action.error) && action.error.code === "port_in_use";

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="mailpit-title">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h2 id="mailpit-title" className="text-lg font-semibold">
            {t("mailpit.title")}
          </h2>
          {missing && !running ? <NotInstalledBadge /> : <StatusBadge status={busy ? (running ? "stopping" : "starting") : status.status} />}
        </div>
        <div className="flex flex-wrap gap-2">
          {running ? (
            <Button onClick={() => run(() => serviceStop("mailpit"))} disabled={busy} icon={<Square />}>
              {t("actions.stop")}
            </Button>
          ) : missing ? (
            <InstallButton pkg="mailpit" name="Mailpit" />
          ) : (
            <Button primary onClick={() => run(() => serviceStart("mailpit"))} disabled={busy || installed.length === 0} icon={<Play />}>
              {t("actions.start")}
            </Button>
          )}
          <Button onClick={() => run(mailpitOpen)} disabled={busy || !running} icon={<Inbox />}>
            {t("mailpit.openInbox")}
          </Button>
          <Button onClick={() => run(() => serviceOpenLog("mailpit"))} disabled={busy} icon={<FileText />}>
            {t("actions.openLog")}
          </Button>
        </div>
      </div>

      {config && (
        <div className="mt-4 space-y-2">
          <label className="flex items-center gap-2.5 text-sm">
            <input
              type="checkbox"
              className="size-4 accent-accent"
              checked={config.enabled}
              disabled={busy}
              onChange={(e) => update({ enabled: e.target.checked })}
            />
            {t("mailpit.enabled")}
          </label>
          <label className="flex items-center gap-2.5 text-sm">
            <input
              type="checkbox"
              className="size-4 accent-accent"
              checked={config.php_mail}
              disabled={busy}
              onChange={(e) => update({ php_mail: e.target.checked })}
            />
            {t("mailpit.phpMail")}
          </label>
        </div>
      )}

      {installed.length > 0 && config && (
        <div className="mt-4 max-w-sm">
          <VersionSelect
            label={t("mailpit.version")}
            newestLabel={t("mailpit.newest", { version: installed[0]?.version })}
            versions={installed.map((v) => v.version)}
            value={config.version}
            disabled={busy}
            onChange={(v) => update({ version: v }, true)}
          />
        </div>
      )}

      {versions.isSuccess && installed.length === 0 && <p className="mt-4 text-sm text-muted">{t("mailpit.notInstalled")}</p>}

      {rt && (
        <>
          <dl className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
            <Fact label={t("mailpit.version")} value={rt.version} />
            <Fact label={t("mailpit.smtpPort")} value={rt.smtpPort} />
            <Fact label={t("mailpit.uiPort")} value={rt.uiPort} />
          </dl>
          <p className="mt-3 text-sm text-muted">{t("mailpit.connectHint", { port: rt.smtpPort })}</p>
        </>
      )}

      <div className="mt-4 space-y-2">
        {!hideDialogError && <ErrorBox error={action.error} />}
        {!action.error && status.status === "failed" && status.error && (
          <ErrorBox error={{ code: "service", message: status.error }} />
        )}
      </div>
    </section>
  );
}

/** If `err` is a busy Mailpit port: moves that port to the alternative, then starts Mailpit. */
export function mailpitConflictRequest(err: unknown, mailpit: MailpitSettings): PortConflictRequest | null {
  const smtp = portConflictOf(err, mailpit.smtp_port);
  const ui = portConflictOf(err, mailpit.ui_port);
  const conflict: PortConflict | null = smtp ?? ui;
  if (!conflict) return null;
  const field = smtp ? "smtp_port" : "ui_port";
  return {
    conflict,
    service: "Mailpit",
    isWeb: false,
    apply: async (alternative) => {
      await settingsUpdate("mailpit", { ...mailpit, [field]: alternative });
      await serviceStart("mailpit");
    },
  };
}
