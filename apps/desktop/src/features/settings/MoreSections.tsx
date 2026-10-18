import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { ChevronRight, FolderOpen } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
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
import { differs, ErrorRow, FormSection, inputClass, NumberInput, Row, Section, SwitchRow } from "@/features/settings/parts";

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

/** Shape of a proxy address (an example, not translated). */
const PROXY_EXAMPLE = "http://proxy:8080";

/** Where the projects live (`[general] www_root`, empty = `<data root>/www`). */
export function ProjectsFolderField({ value, onChange, fallback }: { value: string; onChange: (v: string) => void; fallback: string }) {
  const { t } = useTranslation();
  const browse = async () => {
    const picked = await open({ directory: true, defaultPath: value || fallback });
    if (typeof picked === "string") onChange(picked);
  };
  return (
    <Row label={t("settings.general.www")} hint={t("settings.general.wwwHint", { folder: fallback, interpolation: { escapeValue: false } })} htmlFor="general-www" wide>
      <span className="flex gap-2">
        <input id="general-www" className={`${inputClass} font-mono`} dir="ltr" value={value} placeholder={fallback} onChange={(e) => onChange(e.target.value)} />
        <Button onClick={() => void browse()} icon={<FolderOpen />}>
          {t("settings.browse")}
        </Button>
      </span>
    </Row>
  );
}

/** Database, PostgreSQL and Redis ports (engine and versions are on the Services page). */
export function DatabaseForm({ database, redis, postgresql }: { database: DatabaseSettings; redis: RedisSettings; postgresql: PostgresqlSettings }) {
  const { t } = useTranslation();
  const [db, setDb] = useState(database);
  const [rd, setRd] = useState(redis);
  const [pg, setPg] = useState(postgresql);
  const save = useSaveSections();
  return (
    <FormSection
      id="database"
      title={t("settings.database.title")}
      description={t("settings.database.restart")}
      dirty={differs([db, rd, pg], [database, redis, postgresql])}
      saving={save.isPending}
      saved={save.isSuccess}
      onSubmit={() =>
        save.mutate([
          ["database", db],
          ["redis", rd],
          ["postgresql", pg],
        ])
      }
      onReset={() => {
        setDb(database);
        setRd(redis);
        setPg(postgresql);
      }}
    >
      <Row label={t("settings.database.port")} hint={t("settings.database.portHint")} htmlFor="db-port">
        <NumberInput id="db-port" min={1} max={65535} value={db.port} onChange={(e) => setDb({ ...db, port: Number(e.target.value) })} />
      </Row>
      <Row label={t("settings.database.postgresqlPort")} htmlFor="pg-port">
        <NumberInput id="pg-port" min={1} max={65535} value={pg.port} onChange={(e) => setPg({ ...pg, port: Number(e.target.value) })} />
      </Row>
      <SwitchRow label={t("settings.database.postgresqlEnabled")} checked={pg.enabled} onChange={(enabled) => setPg({ ...pg, enabled })} />
      <Row label={t("settings.database.redisPort")} htmlFor="redis-port">
        <NumberInput id="redis-port" min={1} max={65535} value={rd.port} onChange={(e) => setRd({ ...rd, port: Number(e.target.value) })} />
      </Row>
      <SwitchRow label={t("settings.database.redisEnabled")} checked={rd.enabled} onChange={(enabled) => setRd({ ...rd, enabled })} />
      <ErrorRow error={save.error} />
    </FormSection>
  );
}

/** Mailpit's ports, Start All and whether PHP's mail() goes to it. */
export function MailForm({ initial }: { initial: MailpitSettings }) {
  const { t } = useTranslation();
  const [mail, setMail] = useState(initial);
  const save = useSaveSections();
  return (
    <FormSection
      id="mail"
      title={t("settings.mail.title")}
      description={t("settings.mail.restart")}
      dirty={differs(mail, initial)}
      saving={save.isPending}
      saved={save.isSuccess}
      onSubmit={() => save.mutate([["mailpit", mail]])}
      onReset={() => setMail(initial)}
    >
      <Row label={t("settings.mail.smtpPort")} hint={t("settings.mail.smtpPortHint")} htmlFor="mail-smtp">
        <NumberInput id="mail-smtp" min={1} max={65535} value={mail.smtp_port} onChange={(e) => setMail({ ...mail, smtp_port: Number(e.target.value) })} />
      </Row>
      <Row label={t("settings.mail.uiPort")} htmlFor="mail-ui">
        <NumberInput id="mail-ui" min={1} max={65535} value={mail.ui_port} onChange={(e) => setMail({ ...mail, ui_port: Number(e.target.value) })} />
      </Row>
      <SwitchRow label={t("mailpit.enabled")} checked={mail.enabled} onChange={(enabled) => setMail({ ...mail, enabled })} />
      <SwitchRow label={t("mailpit.phpMail")} checked={mail.php_mail} onChange={(php_mail) => setMail({ ...mail, php_mail })} />
      <ErrorRow error={save.error} />
    </FormSection>
  );
}

/** Language and theme: they apply at once, so no Save. */
export function AppearanceCard() {
  const { t } = useTranslation();
  return (
    <Section id="appearance" title={t("settings.appearance.title")} description={t("settings.appearance.description")}>
      <Row label={t("language.label")} hint={t("settings.appearance.hint")}>
        <div className="w-full">
          <LanguageSelect />
        </div>
      </Row>
      <Row label={t("theme.label")}>
        <div className="w-full">
          <ThemeSelect />
        </div>
      </Row>
    </Section>
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
    <Section id="elsewhere" title={t("settings.elsewhere.title")} description={t("settings.elsewhere.description")}>
      <ul className="grid gap-px bg-line @2xl:grid-cols-2">
        {links.map(([to, title, hint]) => (
          <li key={to} className="bg-surface @2xl:[&:last-child:nth-child(odd)]:col-span-2">
            <Link to={to} className="flex h-full items-center justify-between gap-3 px-5 py-3.5 text-sm outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent">
              <span className="min-w-0">
                <span className="block font-medium">{title}</span>
                <span className="block text-xs text-muted">{hint}</span>
              </span>
              <ChevronRight className="size-4 shrink-0 text-muted rtl:rotate-180" aria-hidden />
            </Link>
          </li>
        ))}
      </ul>
    </Section>
  );
}

/** Download proxy and where Lumina keeps its files. */
export function AdvancedForm({ proxy, path }: { proxy: string; path: string }) {
  const { t } = useTranslation();
  const [value, setValue] = useState(proxy);
  const save = useSaveSections();
  const folder = useMutation({ mutationFn: logsOpenFolder });
  return (
    <FormSection
      id="advanced"
      title={t("settings.advanced.title")}
      dirty={value.trim() !== proxy}
      saving={save.isPending}
      saved={save.isSuccess}
      onSubmit={() => save.mutate([["packages", { proxy: value.trim() }]])}
      onReset={() => setValue(proxy)}
    >
      <Row label={t("settings.advanced.proxy")} hint={t("settings.advanced.proxyHint")} htmlFor="proxy">
        <input id="proxy" className={`${inputClass} font-mono`} dir="ltr" placeholder={PROXY_EXAMPLE} value={value} onChange={(e) => setValue(e.target.value)} />
      </Row>
      <Row label={t("settings.file")}>
        <span className="block truncate font-mono text-xs text-fg-soft @md:text-end" dir="ltr" title={path}>
          {path}
        </span>
      </Row>
      <Row label={t("settings.advanced.logs")}>
        <Button onClick={() => folder.mutate()} icon={<FolderOpen />}>
          {t("logs.openFolder")}
        </Button>
      </Row>
      <ErrorRow error={save.error ?? folder.error} />
    </FormSection>
  );
}
