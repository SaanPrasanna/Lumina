import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check } from "lucide-react";
import { type FormEvent, type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { ConfigWarnings } from "@/components/ConfigWarnings";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { editorOpen, editorPresets } from "@/features/env-path/api";
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

export function SettingsPage() {
  const { t } = useTranslation();
  const settings = useQuery({ queryKey: SETTINGS_KEY, queryFn: settingsGet });

  return (
    <>
      <PageHeader title={t("nav.settings")} />
      <div className="max-w-2xl space-y-6 px-8 pb-8">
        {settings.error ? <ErrorBox error={settings.error} /> : null}
        {settings.data && (
          <>
            <ConfigWarnings warnings={settings.data.warnings} />
            {/* key: reset the forms when the saved config changes elsewhere */}
            <GeneralForm
              key={JSON.stringify(settings.data.config.general)}
              initial={settings.data.config.general}
            />
            <WebForm key={JSON.stringify(settings.data.config.web)} initial={settings.data.config.web} />
            <PhpForm key={JSON.stringify(settings.data.config.php)} initial={settings.data.config.php} />
            <EditorForm
              key={JSON.stringify(settings.data.config.editor)}
              initial={settings.data.config.editor}
            />
            <p className="text-xs text-muted">
              {t("settings.file")}:{" "}
              <span className="font-mono" dir="ltr">
                {settings.data.path}
              </span>
            </p>
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
    <Section title={t("php.title")} onSubmit={() => save.mutate(form)} saving={save.isPending} saved={save.isSuccess}>
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
    </Section>
  );
}

function GeneralForm({ initial }: { initial: GeneralSettings }) {
  const { t } = useTranslation();
  const [form, setForm] = useState(initial);
  const save = useSaveSection("general");
  const toggle = (key: "autostart_services" | "close_to_tray") => (e: React.ChangeEvent<HTMLInputElement>) =>
    setForm({ ...form, [key]: e.target.checked });

  return (
    <Section title={t("settings.general.title")} onSubmit={() => save.mutate(form)} saving={save.isPending} saved={save.isSuccess}>
      <Checkbox label={t("settings.general.autostart")} checked={form.autostart_services} onChange={toggle("autostart_services")} />
      <Checkbox label={t("settings.general.closeToTray")} checked={form.close_to_tray} onChange={toggle("close_to_tray")} />
      <ErrorBox error={save.error} />
    </Section>
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
    <Section title={t("settings.web.title")} onSubmit={() => save.mutate(form)} saving={save.isPending} saved={save.isSuccess}>
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
    </Section>
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
    <Section title={t("settings.editor.title")} onSubmit={submit} saving={save.isPending} saved={save.isSuccess}>
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
      <button type="button" className="text-sm text-accent hover:underline" onClick={() => open.mutate()}>
        {t("editor.openConfigToml")}
      </button>
      <ErrorBox error={save.error ?? open.error} />
    </Section>
  );
}

function Section(props: {
  title: string;
  children: ReactNode;
  onSubmit: () => void;
  saving: boolean;
  saved: boolean;
}) {
  const { t } = useTranslation();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    props.onSubmit();
  };
  return (
    <form onSubmit={submit} className="space-y-4 rounded-xl border border-line p-5">
      <h2 className="text-lg font-semibold">{props.title}</h2>
      {props.children}
      <div className="flex items-center gap-3">
        <button
          type="submit"
          disabled={props.saving}
          className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent-strong disabled:opacity-60"
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
