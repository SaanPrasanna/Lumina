import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check } from "lucide-react";
import { type FormEvent, type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { ConfigWarnings } from "@/components/ConfigWarnings";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { PHP_STATUS_KEY, phpStatus } from "@/features/services/api";
import {
  type EditorSettings,
  type PhpSettings,
  SETTINGS_KEY,
  type Settings,
  settingsGet,
  settingsUpdate,
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
  const php = useQuery({ queryKey: PHP_STATUS_KEY, queryFn: phpStatus });
  const running = php.data?.runtime != null;

  const num = (key: keyof PhpSettings) => (e: React.ChangeEvent<HTMLInputElement>) =>
    setForm({ ...form, [key]: Number(e.target.value) });

  return (
    <Section title={t("php.title")} onSubmit={() => save.mutate(form)} saving={save.isPending} saved={save.isSuccess}>
      <div className="grid gap-4 sm:grid-cols-2">
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

function EditorForm({ initial }: { initial: EditorSettings }) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial.name);
  const [command, setCommand] = useState(initial.command);
  const [args, setArgs] = useState(initial.args.join("\n"));
  const save = useSaveSection("editor");

  const submit = () =>
    save.mutate({
      name,
      command,
      args: args
        .split("\n")
        .map((a) => a.trim())
        .filter(Boolean),
    });

  return (
    <Section title={t("settings.editor.title")} onSubmit={submit} saving={save.isPending} saved={save.isSuccess}>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label={t("settings.editor.name")}>
          <TextInput value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label={t("settings.editor.command")}>
          <TextInput value={command} onChange={(e) => setCommand(e.target.value)} dir="ltr" className="font-mono" />
        </Field>
      </div>
      <Field label={t("settings.editor.args")}>
        <textarea
          value={args}
          onChange={(e) => setArgs(e.target.value)}
          rows={3}
          dir="ltr"
          spellCheck={false}
          className="w-full rounded-md border border-line bg-surface px-3 py-2 font-mono text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent"
        />
      </Field>
      <ErrorBox error={save.error} />
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
