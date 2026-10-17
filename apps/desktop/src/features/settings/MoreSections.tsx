import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, ChevronRight, FolderOpen } from "lucide-react";
import { type FormEvent, type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { ErrorBox } from "@/components/ErrorBox";
import { LanguageSelect } from "@/components/LanguageSelect";
import { ThemeSelect } from "@/components/ThemeSelect";
import { Button } from "@/components/ui";
import { logsOpenFolder } from "@/features/logs/api";
import {
  type DatabaseSettings,
  type MailpitSettings,
  type PostgresqlSettings,
  type RedisSettings,
  SETTINGS_KEY,
  type Settings,
  settingsUpdate,
} from "@/features/settings/api";

/** Saves several sections in order and puts the final (validated) result in the cache. */
function useSaveSections() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (sections: [string, unknown][]) => {
      let last: Settings | undefined;
      for (const [section, value] of sections) last = await settingsUpdate(section, value);
      return last;
    },
    onSuccess: (next) => next && queryClient.setQueryData(SETTINGS_KEY, next),
  });
}

/** A settings card with a Save button. */
export function FormCard(props: { id: string; title: string; children: ReactNode; onSubmit: () => void; saving: boolean; saved: boolean }) {
  const { t } = useTranslation();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    props.onSubmit();
  };
  return (
    <form id={props.id} onSubmit={submit} className="scroll-mt-4 space-y-4 rounded-xl border border-line p-5" aria-labelledby={`${props.id}-title`}>
      <h2 id={`${props.id}-title`} className="text-lg font-semibold">
        {props.title}
      </h2>
      {props.children}
      <div className="flex items-center gap-3">
        <button
          type="submit"
          disabled={props.saving}
          className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-on-accent outline-none hover:bg-accent-strong focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 disabled:opacity-60"
        >
          {props.saving ? t("actions.saving") : t("actions.save")}
        </button>
        {props.saved && (
          <span className="flex items-center gap-1 text-sm text-ok">
            <Check className="size-4" aria-hidden />
            {t("settings.saved")}
          </span>
        )}
      </div>
    </form>
  );
}

/** A settings card without a form. */
export function Card({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section id={id} className="scroll-mt-4 space-y-3 rounded-xl border border-line p-5" aria-labelledby={`${id}-title`}>
      <h2 id={`${id}-title`} className="text-lg font-semibold">
        {title}
      </h2>
      {children}
    </section>
  );
}

/** Shape of a proxy address (an example, not translated). */
const PROXY_EXAMPLE = "http://proxy:8080";

const input =
  "w-full rounded-md border border-line bg-surface px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent";

function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      {children}
      {hint && <span className="text-xs text-muted">{hint}</span>}
    </label>
  );
}

/** Where the projects live (`[general] www_root`, empty = `<data root>/www`). */
export function ProjectsFolderField({ value, onChange, fallback }: { value: string; onChange: (v: string) => void; fallback: string }) {
  const { t } = useTranslation();
  const browse = async () => {
    const picked = await open({ directory: true, defaultPath: value || fallback });
    if (typeof picked === "string") onChange(picked);
  };
  return (
    <Field label={t("settings.general.www")} hint={t("settings.general.wwwHint", { folder: fallback, interpolation: { escapeValue: false } })}>
      <span className="flex gap-2">
        <input className={`${input} font-mono`} dir="ltr" value={value} placeholder={fallback} onChange={(e) => onChange(e.target.value)} />
        <Button onClick={() => void browse()} icon={<FolderOpen />}>
          {t("settings.browse")}
        </Button>
      </span>
    </Field>
  );
}

/** Database, PostgreSQL and Redis ports (engine and versions are on the Services page). */
export function DatabaseForm({
  database,
  redis,
  postgresql,
}: {
  database: DatabaseSettings;
  redis: RedisSettings;
  postgresql: PostgresqlSettings;
}) {
  const { t } = useTranslation();
  const [db, setDb] = useState(database);
  const [rd, setRd] = useState(redis);
  const [pg, setPg] = useState(postgresql);
  const save = useSaveSections();
  const port = "w-full rounded-md border border-line bg-surface px-3 py-2 font-mono text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent";
  return (
    <FormCard
      id="database"
      title={t("settings.database.title")}
      onSubmit={() =>
        save.mutate([
          ["database", db],
          ["redis", rd],
          ["postgresql", pg],
        ])
      }
      saving={save.isPending}
      saved={save.isSuccess}
    >
      <div className="grid items-start gap-4 sm:grid-cols-2">
        <Field label={t("settings.database.port")} hint={t("settings.database.portHint")}>
          <input type="number" required min={1} max={65535} dir="ltr" className={port} value={db.port} onChange={(e) => setDb({ ...db, port: Number(e.target.value) })} />
        </Field>
        <Field label={t("settings.database.redisPort")}>
          <input type="number" required min={1} max={65535} dir="ltr" className={port} value={rd.port} onChange={(e) => setRd({ ...rd, port: Number(e.target.value) })} />
        </Field>
        <Field label={t("settings.database.postgresqlPort")}>
          <input type="number" required min={1} max={65535} dir="ltr" className={port} value={pg.port} onChange={(e) => setPg({ ...pg, port: Number(e.target.value) })} />
        </Field>
      </div>
      <label className="flex items-center gap-2.5 text-sm">
        <input type="checkbox" className="size-4 accent-accent" checked={rd.enabled} onChange={(e) => setRd({ ...rd, enabled: e.target.checked })} />
        {t("settings.database.redisEnabled")}
      </label>
      <label className="flex items-center gap-2.5 text-sm">
        <input type="checkbox" className="size-4 accent-accent" checked={pg.enabled} onChange={(e) => setPg({ ...pg, enabled: e.target.checked })} />
        {t("settings.database.postgresqlEnabled")}
      </label>
      <p className="text-xs text-muted">{t("settings.database.restart")}</p>
      <ErrorBox error={save.error} />
    </FormCard>
  );
}

/** Mailpit's ports, Start All and whether PHP's mail() goes to it. */
export function MailForm({ initial }: { initial: MailpitSettings }) {
  const { t } = useTranslation();
  const [mail, setMail] = useState(initial);
  const save = useSaveSections();
  const port = "w-full rounded-md border border-line bg-surface px-3 py-2 font-mono text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent";
  return (
    <FormCard id="mail" title={t("settings.mail.title")} onSubmit={() => save.mutate([["mailpit", mail]])} saving={save.isPending} saved={save.isSuccess}>
      <div className="grid items-start gap-4 sm:grid-cols-2">
        <Field label={t("settings.mail.smtpPort")} hint={t("settings.mail.smtpPortHint")}>
          <input type="number" required min={1} max={65535} dir="ltr" className={port} value={mail.smtp_port} onChange={(e) => setMail({ ...mail, smtp_port: Number(e.target.value) })} />
        </Field>
        <Field label={t("settings.mail.uiPort")}>
          <input type="number" required min={1} max={65535} dir="ltr" className={port} value={mail.ui_port} onChange={(e) => setMail({ ...mail, ui_port: Number(e.target.value) })} />
        </Field>
      </div>
      <label className="flex items-center gap-2.5 text-sm">
        <input type="checkbox" className="size-4 accent-accent" checked={mail.enabled} onChange={(e) => setMail({ ...mail, enabled: e.target.checked })} />
        {t("mailpit.enabled")}
      </label>
      <label className="flex items-center gap-2.5 text-sm">
        <input type="checkbox" className="size-4 accent-accent" checked={mail.php_mail} onChange={(e) => setMail({ ...mail, php_mail: e.target.checked })} />
        {t("mailpit.phpMail")}
      </label>
      <p className="text-xs text-muted">{t("settings.mail.restart")}</p>
      <ErrorBox error={save.error} />
    </FormCard>
  );
}

export function AppearanceCard() {
  const { t } = useTranslation();
  return (
    <Card id="appearance" title={t("settings.appearance.title")}>
      <div className="grid max-w-md gap-3">
        <LanguageSelect />
        <ThemeSelect />
      </div>
      <p className="text-xs text-muted">{t("settings.appearance.hint")}</p>
    </Card>
  );
}

/** Settings that live with their feature. */
export function ElsewhereCard() {
  const { t } = useTranslation();
  const links: [string, string, string][] = [
    ["/sites", t("settings.elsewhere.https"), t("settings.elsewhere.httpsHint")],
    ["/services", t("settings.elsewhere.versions"), t("settings.elsewhere.versionsHint")],
    ["/env-path", t("settings.elsewhere.path"), t("settings.elsewhere.pathHint")],
    ["/tunnel", t("settings.elsewhere.sharing"), t("settings.elsewhere.sharingHint")],
    ["/profiles", t("settings.elsewhere.profiles"), t("settings.elsewhere.profilesHint")],
  ];
  return (
    <Card id="elsewhere" title={t("settings.elsewhere.title")}>
      <ul className="divide-y divide-line rounded-lg border border-line">
        {links.map(([to, title, hint]) => (
          <li key={to}>
            <Link to={to} className="flex items-center justify-between gap-3 px-4 py-2.5 text-sm outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent">
              <span className="min-w-0">
                <span className="block font-medium">{title}</span>
                <span className="block truncate text-xs text-muted">{hint}</span>
              </span>
              <ChevronRight className="size-4 shrink-0 text-muted rtl:rotate-180" aria-hidden />
            </Link>
          </li>
        ))}
      </ul>
    </Card>
  );
}

/** Download proxy and folders. */
export function AdvancedForm({ proxy, path }: { proxy: string; path: string }) {
  const { t } = useTranslation();
  const [value, setValue] = useState(proxy);
  const save = useSaveSections();
  const folder = useMutation({ mutationFn: logsOpenFolder });
  return (
    <FormCard id="advanced" title={t("settings.advanced.title")} onSubmit={() => save.mutate([["packages", { proxy: value.trim() }]])} saving={save.isPending} saved={save.isSuccess}>
      <Field label={t("settings.advanced.proxy")} hint={t("settings.advanced.proxyHint")}>
        <input className={`${input} font-mono`} dir="ltr" placeholder={PROXY_EXAMPLE} value={value} onChange={(e) => setValue(e.target.value)} />
      </Field>
      <div className="flex flex-wrap items-center gap-2 text-sm">
        <span className="text-muted">{t("settings.file")}:</span>
        <span className="min-w-0 truncate font-mono text-xs" dir="ltr">
          {path}
        </span>
      </div>
      <Button onClick={() => folder.mutate()} icon={<FolderOpen />}>
        {t("logs.openFolder")}
      </Button>
      <ErrorBox error={save.error ?? folder.error} />
    </FormCard>
  );
}
