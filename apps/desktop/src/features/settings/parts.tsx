import { Check } from "lucide-react";
import type { FormEvent, InputHTMLAttributes, ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";

/**
 * Building blocks of the Settings page: every section is a card with a
 * title and short description, its settings are rows (label and hint at the
 * start, control at the end), and a form's Save bar only wakes up when
 * something changed.
 */

/** A settings section without a form. */
export function Section(props: { id: string; title: string; description?: ReactNode; children: ReactNode; footer?: ReactNode }) {
  return (
    <section id={props.id} aria-labelledby={`${props.id}-title`} className="@container scroll-mt-6 rounded-xl border border-line bg-surface">
      <SectionHead id={props.id} title={props.title} description={props.description} />
      <div className="divide-y divide-line">{props.children}</div>
      {props.footer && <div className="flex flex-wrap items-center gap-3 border-t border-line px-5 py-3">{props.footer}</div>}
    </section>
  );
}

/** A settings section that saves: rows plus a Save bar. */
export function FormSection(props: {
  id: string;
  title: string;
  description?: ReactNode;
  children: ReactNode;
  dirty: boolean;
  saving: boolean;
  saved: boolean;
  onSubmit: () => void;
  onReset: () => void;
  /** Shown in the bar after a save (e.g. "Restart PHP to apply"). */
  savedNote?: string | null;
  /** Extra controls at the start of the bar. */
  extra?: ReactNode;
}) {
  const { t } = useTranslation();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (props.dirty) props.onSubmit();
  };
  return (
    <form id={props.id} onSubmit={submit} aria-labelledby={`${props.id}-title`} className="@container scroll-mt-6 rounded-xl border border-line bg-surface">
      <SectionHead id={props.id} title={props.title} description={props.description} />
      <div className="divide-y divide-line">{props.children}</div>
      <div className={`flex flex-wrap items-center justify-between gap-3 rounded-b-xl border-t border-line px-5 py-3 transition-colors ${props.dirty ? "bg-accent-soft" : ""}`}>
        <div className="flex min-w-0 flex-wrap items-center gap-3 text-sm">
          {props.extra}
          <span role="status" className="min-w-0">
            {props.dirty ? (
              <span className="font-medium text-accent-ink">{t("settings.unsaved")}</span>
            ) : props.saved ? (
              <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
                <span className="flex items-center gap-1 text-ok">
                  <Check className="size-4" aria-hidden />
                  {t("settings.saved")}
                </span>
                {props.savedNote && <span className="text-warn">{props.savedNote}</span>}
              </span>
            ) : null}
          </span>
        </div>
        <div className="flex gap-2">
          {props.dirty && (
            <button type="button" onClick={props.onReset} disabled={props.saving} className={secondaryButton}>
              {t("settings.reset")}
            </button>
          )}
          <button type="submit" disabled={!props.dirty || props.saving} className={primaryButton}>
            {props.saving ? t("actions.saving") : t("actions.save")}
          </button>
        </div>
      </div>
    </form>
  );
}

function SectionHead({ id, title, description }: { id: string; title: string; description?: ReactNode }) {
  return (
    <header className="border-b border-line px-5 py-4">
      <h2 id={`${id}-title`} className="text-base font-semibold">
        {title}
      </h2>
      {description && <p className="mt-0.5 text-sm text-muted">{description}</p>}
    </header>
  );
}

/**
 * One setting. `wide` puts the control under the label (folders, text
 * areas); otherwise label and control share the row on wide windows.
 */
export function Row(props: { label: string; hint?: ReactNode; htmlFor?: string; wide?: boolean; children: ReactNode }) {
  const label = props.htmlFor ? (
    <label htmlFor={props.htmlFor} className="text-sm font-medium">
      {props.label}
    </label>
  ) : (
    <span className="text-sm font-medium">{props.label}</span>
  );
  return (
    <div className={`gap-x-8 gap-y-2 px-5 py-4 ${props.wide ? "grid" : "grid @md:grid-cols-[minmax(0,1fr)_minmax(9rem,min(40%,20rem))] @md:items-center"}`}>
      <div className="min-w-0">
        {label}
        {props.hint && <p className="mt-0.5 text-xs text-muted">{props.hint}</p>}
      </div>
      <div className={`min-w-0 ${props.wide ? "" : "flex @md:w-full @md:justify-end"}`}>{props.children}</div>
    </div>
  );
}

/** A switch row: the whole row toggles. */
export function SwitchRow(props: { label: string; hint?: ReactNode; checked: boolean; disabled?: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="flex cursor-pointer items-center justify-between gap-6 px-5 py-4 hover:bg-hover/50 has-disabled:cursor-default">
      <span className="min-w-0">
        <span className="block text-sm font-medium">{props.label}</span>
        {props.hint && <span className="mt-0.5 block text-xs text-muted">{props.hint}</span>}
      </span>
      <Switch checked={props.checked} disabled={props.disabled} onChange={props.onChange} />
    </label>
  );
}

/** An on/off switch (a styled checkbox with the switch role). */
export function Switch(props: { checked: boolean; disabled?: boolean; onChange: (checked: boolean) => void; label?: string }) {
  return (
    <span className="relative inline-flex shrink-0">
      <input
        type="checkbox"
        role="switch"
        aria-label={props.label}
        checked={props.checked}
        disabled={props.disabled}
        onChange={(e) => props.onChange(e.target.checked)}
        className="peer absolute inset-0 z-10 cursor-pointer opacity-0 disabled:cursor-default"
      />
      <span
        aria-hidden
        className="h-5 w-9 rounded-full border border-muted/40 bg-muted/25 transition-colors peer-checked:border-accent peer-checked:bg-accent peer-focus-visible:ring-2 peer-focus-visible:ring-accent peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-surface peer-disabled:opacity-50"
      />
      <span
        aria-hidden
        className="absolute start-0.5 top-0.5 size-4 rounded-full bg-white shadow-sm ring-1 ring-black/10 transition-[translate] peer-checked:translate-x-4 rtl:peer-checked:-translate-x-4"
      />
    </span>
  );
}

export const inputClass =
  "w-full rounded-md border border-line bg-surface px-3 py-2 text-sm outline-none transition-colors hover:border-muted/50 focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/30";

/** A port or other number: compact, monospace. */
export function NumberInput({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input type="number" required dir="ltr" {...props} className={`${inputClass.replace("w-full", "w-36")} font-mono ${className ?? ""}`} />;
}

export function TextInput({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input type="text" {...props} className={`${inputClass} ${className ?? ""}`} />;
}

export const primaryButton =
  "rounded-lg bg-accent px-4 py-1.5 text-sm font-medium text-on-accent outline-none transition-colors hover:bg-accent-strong focus-visible:ring-3 focus-visible:ring-accent/50 disabled:cursor-default disabled:opacity-40 disabled:hover:bg-accent";

export const secondaryButton =
  "rounded-lg border border-line px-3 py-1.5 text-sm font-medium outline-none transition-colors hover:bg-hover focus-visible:ring-3 focus-visible:ring-accent/50 disabled:opacity-50";

/** Whether a form differs from what is saved. */
export const differs = (a: unknown, b: unknown) => JSON.stringify(a) !== JSON.stringify(b);

/** An error inside a section, aligned with its rows. */
export function ErrorRow({ error }: { error: Parameters<typeof ErrorBox>[0]["error"] }) {
  if (!error) return null;
  return (
    <div className="px-5 py-3">
      <ErrorBox error={error} />
    </div>
  );
}
