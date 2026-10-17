import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type ReactNode, useState } from "react";
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
import {
  AdvancedForm,
  AppearanceCard,
  DatabaseForm,
  MailForm,
  ElsewhereCard,
  FormCard,
  ProjectsFolderField,
} from "@/features/settings/MoreSections";
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
  ["elsewhere", "settings.elsewhere.title"],
  ["advanced", "settings.advanced.title"],
  ["updates", "settings.updates.title"],
];

export function SettingsPage() {
  const { t } = useTranslation();
  const settings = useQuery({ queryKey: SETTINGS_KEY, queryFn: settingsGet });
  const bootstrap = useQuery({ queryKey: BOOTSTRAP_QUERY_KEY, queryFn: bootstrapStatus });
  const root = bootstrap.data?.dataRoot ?? "";
  const defaultWww = root ? `${root}${root.includes("\\") ? "\\" : "/"}www` : "";

  return (
    <>
      <PageHeader title={t("nav.settings")} />
      <div className="max-w-2xl space-y-6 px-8 pb-8">
        <nav aria-label={t("settings.index")} className="-mt-2 flex flex-wrap gap-x-3 gap-y-1 text-sm">
          {SECTIONS.map(([id, key]) => (
            <a key={id} href={`#${id}`} onClick={(e) => { e.preventDefault(); document.getElementById(id)?.scrollIntoView({ block: "start" }); }} className="rounded text-accent-ink outline-none hover:underline focus-visible:ring-2 focus-visible:ring-accent">
              {t(key)}
            </a>
          ))}
        </nav>
        {settings.error ? <ErrorBox error={settings.error} /> : null}
        {settings.data && (
          <>
            <ConfigWarnings warnings={settings.data.warnings} />
            {/* key: reset the forms when the saved config changes elsewhere */}
            <GeneralForm
              key={JSON.stringify(settings.data.config.general)}
              initial={settings.data.config.general}
              defaultWww={defaultWww}
            />
            <WebForm key={JSON.stringify(settings.data.config.web)} initial={settings.data.config.web} />
            <PhpForm key={JSON.stringify(settings.data.config.php)} initial={settings.data.config.php} />
            <DatabaseForm
              key={JSON.stringify([settings.data.config.database, settings.data.config.redis, settings.data.config.postgresql])}
              database={settings.data.config.database}
              redis={settings.data.config.redis}
              postgresql={settings.data.config.postgresql}
            />
            <MailForm key={JSON.stringify(settings.data.config.mailpit)} initial={settings.data.config.mailpit} />
            <EditorForm
              key={JSON.stringify(settings.data.config.editor)}
              initial={settings.data.config.editor}
            />
            <AppearanceCard />
            <ElsewhereCard />
            <AdvancedForm
              key={JSON.stringify(settings.data.config.packages)}
              proxy={(settings.data.config.packages as { proxy?: string } | undefined)?.proxy ?? ""}
              path={settings.data.path}
            />
            <UpdatesCard />
          </>
        )}
      </div>
    </>
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

function PhpForm({ initial }: { initial: PhpSettings }) {
  const { t } = useTranslation();
  const [form, setForm] = useState(initial);
  const save = useSaveSection("php");
  const stack = useQuery({ queryKey: SERVICES_KEY, queryFn: servicesStatus });
  const running = stack.data?.php.runtime != null;

  const num = (key: keyof PhpSettings) => (e: React.ChangeEvent<HTMLInputElement>) =>
    setForm({ ...form, [key]: Number(e.target.value) });

  return (
    <FormCard id="php" title={t("php.title")} onSubmit={() => save.mutate(form)} saving={save.isPending} saved={save.isSuccess}>
      <div className="grid items-start gap-4 sm:grid-cols-2">
        <Field label={t("settings.php.port")} hint={t("settings.php.portHint")}>
          <NumberInput value={form.fcgi_port} onChange={num("fcgi_port")} min={1} max={65535} />
        </Field>
        <Field label={t("settings.php.fallbackPort")}>
          <NumberInput value={form.fcgi_fallback_port} onChange={num("fcgi_fallback_port")} min={1} max={65535} />
        </Field>
        <Field label={t("settings.php.children")} hint={t("settings.php.childrenHint")}>
          <NumberInput value={form.fcgi_children} onChange={num("fcgi_children")} min={1} max={256} />
        </Field>
        <Field label={t("settings.php.maxRequests")} hint={t("settings.php.maxRequestsHint")}>
          <NumberInput value={form.fcgi_max_requests} onChange={num("fcgi_max_requests")} min={0} />
        </Field>
      </div>
      {save.isSuccess && running && <p className="text-sm text-warn">{t("settings.restartToApply")}</p>}
      <ErrorBox error={save.error} />
    </FormCard>
  );
}

function GeneralForm({ initial, defaultWww }: { initial: GeneralSettings; defaultWww: string }) {
  const { t } = useTranslation();
  const [form, setForm] = useState(initial);
  const save = useSaveSection("general");
  const toggle = (key: "autostart_services" | "close_to_tray" | "start_on_login") => (e: React.ChangeEvent<HTMLInputElement>) =>
    setForm({ ...form, [key]: e.target.checked });

  return (
    <FormCard id="general" title={t("settings.general.title")} onSubmit={() => save.mutate(form)} saving={save.isPending} saved={save.isSuccess}>
      <ProjectsFolderField value={form.www_root} onChange={(www_root) => setForm({ ...form, www_root })} fallback={defaultWww} />
      <Checkbox label={t("settings.general.startOnLogin")} checked={form.start_on_login} onChange={toggle("start_on_login")} />
      <Checkbox label={t("settings.general.autostart")} checked={form.autostart_services} onChange={toggle("autostart_services")} />
      <Checkbox label={t("settings.general.closeToTray")} checked={form.close_to_tray} onChange={toggle("close_to_tray")} />
      <ErrorBox error={save.error} />
    </FormCard>
  );
}

function WebForm({ initial }: { initial: WebSettings }) {
  const { t } = useTranslation();
  const [form, setForm] = useState(initial);
  const save = useSaveSection("web");
  const stack = useQuery({ queryKey: SERVICES_KEY, queryFn: servicesStatus });
  const running = stack.data?.web.runtime != null;
  const num = (key: "http_port" | "https_port") => (e: React.ChangeEvent<HTMLInputElement>) =>
    setForm({ ...form, [key]: Number(e.target.value) });

  return (
    <FormCard id="web" title={t("settings.web.title")} onSubmit={() => save.mutate(form)} saving={save.isPending} saved={save.isSuccess}>
      <div className="grid items-start gap-4 sm:grid-cols-2">
        <Field label={t("settings.web.httpPort")} hint={t("settings.web.portHint")}>
          <NumberInput value={form.http_port} onChange={num("http_port")} min={1} max={65535} />
        </Field>
        <Field label={t("settings.web.httpsPort")}>
          <NumberInput value={form.https_port} onChange={num("https_port")} min={1} max={65535} />
        </Field>
      </div>
      {save.isSuccess && running && <p className="text-sm text-warn">{t("settings.restartWebToApply")}</p>}
      <ErrorBox error={save.error} />
    </FormCard>
  );
}

function Checkbox(props: { label: string; checked: boolean; onChange: (e: React.ChangeEvent<HTMLInputElement>) => void }) {
  return (
    <label className="flex items-center gap-2.5 text-sm">
      <input type="checkbox" checked={props.checked} onChange={props.onChange} className="size-4 accent-accent" />
      {props.label}
    </label>
  );
}

function EditorForm({ initial }: { initial: EditorSettings }) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial.name);
  const [command, setCommand] = useState(initial.command);
  const [args, setArgs] = useState(initial.args.join("\n"));
  const [argsLine, setArgsLine] = useState(initial.args_line.join("\n"));
  const save = useSaveSection("editor");
  const presets = useQuery({ queryKey: ["editor", "presets"], queryFn: editorPresets });
  const open = useMutation({ mutationFn: () => editorOpen({ kind: "config", file: "lumina" }) });
  const lines = (text: string) =>
    text
      .split("\n")
      .map((a) => a.trim())
      .filter(Boolean);

  const submit = () => save.mutate({ name, command, args: lines(args), args_line: lines(argsLine) });
  const applyPreset = (id: string) => {
    const preset = presets.data?.find((p) => p.id === id);
    if (!preset) return;
    setName(preset.name);
    setCommand(preset.command);
    setArgs(preset.args.join("\n"));
    setArgsLine(preset.argsLine.join("\n"));
  };
  const area = "w-full rounded-md border border-line bg-surface px-3 py-2 font-mono text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent";

  return (
    <FormCard id="editor" title={t("settings.editor.title")} onSubmit={submit} saving={save.isPending} saved={save.isSuccess}>
      <Field label={t("editor.preset")} hint={presets.data?.length === 0 ? t("editor.noPresets") : t("editor.presetHint")}>
        <select
          className="w-full rounded-md border border-line bg-surface px-3 py-2"
          value=""
          onChange={(e) => applyPreset(e.target.value)}
          disabled={!presets.data?.length}
        >
          <option value="">{t("editor.choosePreset")}</option>
          {presets.data?.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </Field>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label={t("settings.editor.name")}>
          <TextInput value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label={t("settings.editor.command")}>
          <TextInput value={command} onChange={(e) => setCommand(e.target.value)} dir="ltr" className="font-mono" />
        </Field>
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label={t("settings.editor.args")}>
          <textarea value={args} onChange={(e) => setArgs(e.target.value)} rows={3} dir="ltr" spellCheck={false} className={area} />
        </Field>
        <Field label={t("editor.argsLine")}>
          <textarea value={argsLine} onChange={(e) => setArgsLine(e.target.value)} rows={3} dir="ltr" spellCheck={false} className={area} />
        </Field>
      </div>
      <button type="button" className="text-sm text-accent-ink hover:underline" onClick={() => open.mutate()}>
        {t("editor.openConfigToml")}
      </button>
      <ErrorBox error={save.error ?? open.error} />
    </FormCard>
  );
}

function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      {children}
      {hint && <span className="text-xs text-muted">{hint}</span>}
    </label>
  );
}

const inputClass =
  "w-full rounded-md border border-line bg-surface px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent";

function NumberInput(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return <input type="number" required {...props} dir="ltr" className={`${inputClass} font-mono`} />;
}

function TextInput({ className, ...props }: React.InputHTMLAttributes<HTMLInputElement>) {
  return <input type="text" required {...props} className={`${inputClass} ${className ?? ""}`} />;
}
