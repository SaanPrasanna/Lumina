import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileCog } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ConfigWarnings } from "@/components/ConfigWarnings";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { editorOpen, editorPresets } from "@/features/env-path/api";
import { BOOTSTRAP_QUERY_KEY, bootstrapStatus } from "@/features/onboarding/api";
import { SERVICES_KEY, servicesStatus } from "@/features/services/api";
import {
  type EditorSettings,
  type GeneralSettings,
  type PhpSettings,
  SETTINGS_KEY,
  type Settings,
  settingsGet,
  settingsUpdate,
  type WebSettings,
} from "@/features/settings/api";
import { AdvancedForm, AppearanceCard, DatabaseForm, ElsewhereCard, MailForm, ProjectsFolderField } from "@/features/settings/MoreSections";
import { differs, ErrorRow, FormSection, inputClass, NumberInput, Row, SwitchRow, secondaryButton, TextInput } from "@/features/settings/parts";
import { UpdatesCard } from "@/features/updates/UpdatesCard";

/** In-page index entries: section id → title key. */
const SECTIONS: [string, string][] = [
  ["general", "settings.general.title"],
  ["web", "settings.web.title"],
  ["php", "php.title"],
  ["database", "settings.database.title"],
  ["mail", "settings.mail.title"],
  ["editor", "settings.editor.title"],
  ["appearance", "settings.appearance.title"],
  ["advanced", "settings.advanced.title"],
  ["elsewhere", "settings.elsewhere.title"],
  ["updates", "settings.updates.title"],
];

export function SettingsPage() {
  const { t } = useTranslation();
  const settings = useQuery({ queryKey: SETTINGS_KEY, queryFn: settingsGet });
  const bootstrap = useQuery({ queryKey: BOOTSTRAP_QUERY_KEY, queryFn: bootstrapStatus });
  const root = bootstrap.data?.dataRoot ?? "";
  const defaultWww = root ? `${root}${root.includes("\\") ? "\\" : "/"}www` : "";
  const active = useActiveSection(settings.data !== undefined);
  const config = settings.data?.config;

  return (
    <>
      <PageHeader title={t("nav.settings")} />
      <div className="flex items-start gap-6 px-8 pb-10 xl:gap-8">
        <SectionIndex active={active} />
        <div className="min-w-0 max-w-5xl flex-1 space-y-6">
          <SectionIndex active={active} compact />
          {settings.error ? <ErrorBox error={settings.error} /> : null}
          {settings.data && config && (
            <>
              <ConfigWarnings warnings={settings.data.warnings} />
              {/* key: reset the forms when the saved config changes elsewhere */}
              <GeneralForm key={JSON.stringify(config.general)} initial={config.general} defaultWww={defaultWww} />
              <WebForm key={JSON.stringify(config.web)} initial={config.web} />
              <PhpForm key={JSON.stringify(config.php)} initial={config.php} />
              <DatabaseForm
                key={JSON.stringify([config.database, config.redis, config.postgresql])}
                database={config.database}
                redis={config.redis}
                postgresql={config.postgresql}
              />
              <MailForm key={JSON.stringify(config.mailpit)} initial={config.mailpit} />
              <EditorForm key={JSON.stringify(config.editor)} initial={config.editor} />
              <AppearanceCard />
              <AdvancedForm
                key={JSON.stringify(config.packages)}
                proxy={(config.packages as { proxy?: string } | undefined)?.proxy ?? ""}
                path={settings.data.path}
              />
              <ElsewhereCard />
              <UpdatesCard />
            </>
          )}
        </div>
      </div>
    </>
  );
}

/** The section in view, for the index: the last one whose top has scrolled
 *  past the top of the page, or the last one once the page is at its end. */
function useActiveSection(ready: boolean) {
  const [active, setActive] = useState(SECTIONS[0][0]);
  useEffect(() => {
    const main = document.querySelector("main");
    if (!ready || !main) return;
    const update = () => {
      const top = main.getBoundingClientRect().top + 120;
      const atEnd = main.scrollTop + main.clientHeight >= main.scrollHeight - 4;
      let current = SECTIONS[0][0];
      for (const [id] of SECTIONS) {
        const el = document.getElementById(id);
        if (el && (atEnd ? el.getBoundingClientRect().top < main.getBoundingClientRect().bottom : el.getBoundingClientRect().top <= top)) current = id;
      }
      setActive(current);
    };
    update();
    main.addEventListener("scroll", update, { passive: true });
    return () => main.removeEventListener("scroll", update);
  }, [ready]);
  return active;
}

/** Sections index: a sticky list beside the page on wide windows, a row of chips above it otherwise. */
function SectionIndex({ active, compact }: { active: string; compact?: boolean }) {
  const { t } = useTranslation();
  const go = (id: string) => (e: React.MouseEvent) => {
    e.preventDefault();
    document.getElementById(id)?.scrollIntoView({ block: "start", behavior: "smooth" });
  };
  if (compact) {
    return (
      <nav aria-label={t("settings.index")} className="flex flex-wrap gap-1.5 lg:hidden">
        {SECTIONS.map(([id, key]) => (
          <a
            key={id}
            href={`#${id}`}
            onClick={go(id)}
            className={`rounded-full border px-3 py-1 text-xs font-medium outline-none focus-visible:ring-2 focus-visible:ring-accent ${
              active === id ? "border-accent/50 bg-accent-soft text-accent-ink" : "border-line text-muted hover:bg-hover hover:text-fg"
            }`}
          >
            {t(key)}
          </a>
        ))}
      </nav>
    );
  }
  return (
    <nav aria-label={t("settings.index")} className="sticky top-6 hidden w-44 shrink-0 lg:block xl:w-52">
      <ul className="space-y-0.5 border-s border-line">
        {SECTIONS.map(([id, key]) => (
          <li key={id}>
            <a
              href={`#${id}`}
              onClick={go(id)}
              aria-current={active === id ? "true" : undefined}
              className={`-ms-px block border-s-2 py-1.5 ps-4 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-accent ${
                active === id ? "border-accent font-medium text-fg" : "border-transparent text-muted hover:border-line hover:text-fg"
              }`}
            >
              {t(key)}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
}

/** Saves one config section and puts the server's (validated) result in the cache. */
function useSaveSection(section: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (value: unknown) => settingsUpdate(section, value),
    onSuccess: (next: Settings) => queryClient.setQueryData(SETTINGS_KEY, next),
  });
}

function GeneralForm({ initial, defaultWww }: { initial: GeneralSettings; defaultWww: string }) {
  const { t } = useTranslation();
  const [form, setForm] = useState(initial);
  const save = useSaveSection("general");
  const toggle = (key: "autostart_services" | "close_to_tray" | "start_on_login") => (checked: boolean) => setForm({ ...form, [key]: checked });

  return (
    <FormSection
      id="general"
      title={t("settings.general.title")}
      description={t("settings.general.description")}
      dirty={differs(form, initial)}
      saving={save.isPending}
      saved={save.isSuccess}
      onSubmit={() => save.mutate(form)}
      onReset={() => setForm(initial)}
    >
      <ProjectsFolderField value={form.www_root} onChange={(www_root) => setForm({ ...form, www_root })} fallback={defaultWww} />
      <SwitchRow label={t("settings.general.startOnLogin")} checked={form.start_on_login} onChange={toggle("start_on_login")} />
      <SwitchRow label={t("settings.general.autostart")} checked={form.autostart_services} onChange={toggle("autostart_services")} />
      <SwitchRow label={t("settings.general.closeToTray")} checked={form.close_to_tray} onChange={toggle("close_to_tray")} />
      <ErrorRow error={save.error} />
    </FormSection>
  );
}

function WebForm({ initial }: { initial: WebSettings }) {
  const { t } = useTranslation();
  const [form, setForm] = useState(initial);
  const save = useSaveSection("web");
  const stack = useQuery({ queryKey: SERVICES_KEY, queryFn: servicesStatus });
  const running = stack.data?.web.runtime != null;
  const num = (key: "http_port" | "https_port") => (e: React.ChangeEvent<HTMLInputElement>) => setForm({ ...form, [key]: Number(e.target.value) });

  return (
    <FormSection
      id="web"
      title={t("settings.web.title")}
      description={t("settings.web.portHint")}
      dirty={differs(form, initial)}
      saving={save.isPending}
      saved={save.isSuccess}
      savedNote={running ? t("settings.restartWebToApply") : null}
      onSubmit={() => save.mutate(form)}
      onReset={() => setForm(initial)}
    >
      <Row label={t("settings.web.httpPort")} htmlFor="web-http">
        <NumberInput id="web-http" value={form.http_port} onChange={num("http_port")} min={1} max={65535} />
      </Row>
      <Row label={t("settings.web.httpsPort")} htmlFor="web-https">
        <NumberInput id="web-https" value={form.https_port} onChange={num("https_port")} min={1} max={65535} />
      </Row>
      <ErrorRow error={save.error} />
    </FormSection>
  );
}

function PhpForm({ initial }: { initial: PhpSettings }) {
  const { t } = useTranslation();
  const [form, setForm] = useState(initial);
  const save = useSaveSection("php");
  const stack = useQuery({ queryKey: SERVICES_KEY, queryFn: servicesStatus });
  const running = stack.data?.php.runtime != null;
  const num = (key: keyof PhpSettings) => (e: React.ChangeEvent<HTMLInputElement>) => setForm({ ...form, [key]: Number(e.target.value) });

  return (
    <FormSection
      id="php"
      title={t("php.title")}
      description={t("settings.php.description")}
      dirty={differs(form, initial)}
      saving={save.isPending}
      saved={save.isSuccess}
      savedNote={running ? t("settings.restartToApply") : null}
      onSubmit={() => save.mutate(form)}
      onReset={() => setForm(initial)}
    >
      <Row label={t("settings.php.port")} htmlFor="php-port">
        <NumberInput id="php-port" value={form.fcgi_port} onChange={num("fcgi_port")} min={1} max={65535} />
      </Row>
      <Row label={t("settings.php.fallbackPort")} hint={t("settings.php.portHint")} htmlFor="php-fallback">
        <NumberInput id="php-fallback" value={form.fcgi_fallback_port} onChange={num("fcgi_fallback_port")} min={1} max={65535} />
      </Row>
      <Row label={t("settings.php.children")} hint={t("settings.php.childrenHint")} htmlFor="php-children">
        <NumberInput id="php-children" value={form.fcgi_children} onChange={num("fcgi_children")} min={1} max={256} />
      </Row>
      <Row label={t("settings.php.maxRequests")} hint={t("settings.php.maxRequestsHint")} htmlFor="php-max">
        <NumberInput id="php-max" value={form.fcgi_max_requests} onChange={num("fcgi_max_requests")} min={0} />
      </Row>
      <ErrorRow error={save.error} />
    </FormSection>
  );
}

function EditorForm({ initial }: { initial: EditorSettings }) {
  const { t } = useTranslation();
  const start = { name: initial.name, command: initial.command, args: initial.args.join("\n"), argsLine: initial.args_line.join("\n") };
  const [form, setForm] = useState(start);
  const save = useSaveSection("editor");
  const presets = useQuery({ queryKey: ["editor", "presets"], queryFn: editorPresets });
  const open = useMutation({ mutationFn: () => editorOpen({ kind: "config", file: "lumina" }) });
  const lines = (text: string) =>
    text
      .split("\n")
      .map((a) => a.trim())
      .filter(Boolean);

  const submit = () => save.mutate({ name: form.name, command: form.command, args: lines(form.args), args_line: lines(form.argsLine) });
  const applyPreset = (id: string) => {
    const preset = presets.data?.find((p) => p.id === id);
    if (preset) setForm({ name: preset.name, command: preset.command, args: preset.args.join("\n"), argsLine: preset.argsLine.join("\n") });
  };
  const area = `${inputClass} font-mono`;

  return (
    <FormSection
      id="editor"
      title={t("settings.editor.title")}
      description={t("settings.editor.description")}
      dirty={differs(form, start)}
      saving={save.isPending}
      saved={save.isSuccess}
      onSubmit={submit}
      onReset={() => setForm(start)}
      extra={
        <button type="button" className={`${secondaryButton} flex items-center gap-1.5 [&_svg]:size-4`} onClick={() => open.mutate()}>
          <FileCog aria-hidden />
          {t("editor.openConfigToml")}
        </button>
      }
    >
      <Row label={t("editor.preset")} hint={presets.data?.length === 0 ? t("editor.noPresets") : t("editor.presetHint")} htmlFor="editor-preset">
        <select id="editor-preset" className={inputClass} value="" onChange={(e) => applyPreset(e.target.value)} disabled={!presets.data?.length}>
          <option value="">{t("editor.choosePreset")}</option>
          {presets.data?.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </Row>
      <Row label={t("settings.editor.name")} htmlFor="editor-name">
        <TextInput id="editor-name" required value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
      </Row>
      <Row label={t("settings.editor.command")} htmlFor="editor-command">
        <TextInput id="editor-command" required value={form.command} onChange={(e) => setForm({ ...form, command: e.target.value })} dir="ltr" className="font-mono" />
      </Row>
      <div className="grid gap-x-6 gap-y-4 px-5 py-4 @2xl:grid-cols-2">
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("settings.editor.args")}</span>
          <textarea value={form.args} onChange={(e) => setForm({ ...form, args: e.target.value })} rows={3} dir="ltr" spellCheck={false} className={area} />
        </label>
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("editor.argsLine")}</span>
          <textarea value={form.argsLine} onChange={(e) => setForm({ ...form, argsLine: e.target.value })} rows={3} dir="ltr" spellCheck={false} className={area} />
        </label>
      </div>
      <ErrorRow error={save.error ?? open.error} />
    </FormSection>
  );
}
