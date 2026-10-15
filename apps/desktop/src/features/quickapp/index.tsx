import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { Code, ExternalLink, FolderOpen, Rocket, Trash2, X } from "lucide-react";
import { type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button, Fact } from "@/components/ui";
import { editorOpen } from "@/features/env-path/api";
import { formatBytes } from "@/features/packages/api";
import {
  freeName,
  isValidName,
  type Job,
  type Need,
  type OutputEvent,
  QUICKAPP_KEY,
  type QuickAppOverview,
  quickappCancel,
  quickappCreate,
  quickappDeletePartial,
  quickappDismiss,
  quickappOverview,
  type Recipe,
  type Step,
  toName,
} from "@/features/quickapp/api";
import { siteOpen, siteOpenFolder } from "@/features/sites/api";

/** QuickApp: create a Laravel, WordPress, ... project in one click. */
export function QuickAppPage() {
  const { t } = useTranslation();
  const overview = useQuery({ queryKey: QUICKAPP_KEY, queryFn: quickappOverview });
  const [selected, setSelected] = useState<string | null>(null);
  const data = overview.data;
  const job = data?.job ?? null;
  const recipe = data?.recipes.find((r) => r.id === selected) ?? null;

  return (
    <>
      <PageHeader title={t("nav.quickapp")} />
      <div className="space-y-4 px-8 pb-8">
        <p className="-mt-2 text-sm text-muted">{t("quickapp.intro")}</p>
        <ErrorBox error={overview.error} />
        {job && data && <JobPanel job={job} overview={data} />}
        {data && (
          <>
            <ul className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3" aria-label={t("quickapp.recipes")}>
              {data.recipes.map((r) => (
                <li key={r.id}>
                  <button
                    type="button"
                    aria-pressed={r.id === selected}
                    onClick={() => setSelected(r.id === selected ? null : r.id)}
                    className={`h-full w-full rounded-xl border p-4 text-start outline-none focus-visible:ring-2 focus-visible:ring-accent ${
                      r.id === selected ? "border-accent bg-accent/5" : "border-line hover:bg-hover"
                    }`}
                  >
                    <span className="block font-semibold">{r.name}</span>
                    <span className="mt-1 block text-sm text-muted">
                      {t(`quickapp.recipe.${r.id}`, { defaultValue: r.description })}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
            {recipe && (
              <CreateForm
                key={recipe.id}
                recipe={recipe}
                overview={data}
                busy={job?.state === "running"}
                onCreated={() => setSelected(null)}
              />
            )}
          </>
        )}
      </div>
    </>
  );
}

function useQuickAppAction() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () => queryClient.invalidateQueries({ queryKey: QUICKAPP_KEY }),
  });
}

/** The needs of `recipe` with the chosen options. */
function needsFor(recipe: Recipe, options: Record<string, string>): Need[] {
  const needs = new Set(recipe.needs);
  for (const option of recipe.options) {
    const choice = option.choices.find((c) => c.value === options[option.id]);
    for (const need of choice?.needs ?? []) needs.add(need);
  }
  return [...needs];
}

function CreateForm(props: { recipe: Recipe; overview: QuickAppOverview; busy: boolean; onCreated: () => void }) {
  const { recipe, overview, busy } = props;
  const { t } = useTranslation();
  const action = useQuickAppAction();
  const [name, setName] = useState(() => freeName(recipe.id === "php" ? "my-site" : `${recipe.id}-app`, overview.taken));
  const [options, setOptions] = useState<Record<string, string>>(() =>
    Object.fromEntries(recipe.options.map((o) => [o.id, o.default || (o.choices[0]?.value ?? "")])),
  );
  const finalName = name.replace(/-+$/, "");
  const valid = isValidName(finalName);
  const taken = overview.taken.includes(finalName);
  const toInstall = needsFor(recipe, options).filter((n) => overview.missing.includes(n));
  const sep = overview.www.includes("\\") ? "\\" : "/";

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!valid || taken || busy) return;
    action.mutate(() => quickappCreate(recipe.id, finalName, options), { onSuccess: props.onCreated });
  };

  return (
    <form onSubmit={submit} className="space-y-4 rounded-xl border border-line p-5" aria-labelledby="quickapp-form-title">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 id="quickapp-form-title" className="text-lg font-semibold">
          {t("quickapp.newProject", { recipe: recipe.name })}
        </h2>
        {recipe.homepage && (
          <a href={recipe.homepage} target="_blank" rel="noreferrer" className="text-sm text-accent-ink hover:underline" dir="ltr">
            {recipe.homepage.replace(/^https:\/\//, "")}
          </a>
        )}
      </div>

      <div className="flex flex-wrap gap-4">
        <div className="flex min-w-56 flex-1 flex-col gap-1.5 text-sm">
          <label htmlFor="quickapp-name" className="font-medium">
            {t("quickapp.name")}
          </label>
          <input
            id="quickapp-name"
            className="rounded-md border border-line bg-surface px-3 py-2 font-mono"
            dir="ltr"
            value={name}
            onChange={(e) => setName(toName(e.target.value))}
            aria-invalid={!valid || taken}
            aria-describedby="quickapp-name-hint"
            spellCheck={false}
            autoFocus
          />
          <span id="quickapp-name-hint" className={`text-xs ${!valid || taken ? "text-danger" : "text-muted"}`}>
            {!valid ? t("quickapp.nameRule") : taken ? t("quickapp.nameTaken") : t("quickapp.nameHint")}
          </span>
        </div>
        {recipe.options.map((option) => (
          <label key={option.id} className="flex min-w-48 flex-col gap-1.5 text-sm">
            <span className="font-medium">{t(`quickapp.option.${option.id}`, { defaultValue: option.label })}</span>
            <select
              className="rounded-md border border-line bg-surface px-3 py-2"
              value={options[option.id]}
              onChange={(e) => setOptions((o) => ({ ...o, [option.id]: e.target.value }))}
            >
              {option.choices.map((c) => (
                <option key={c.value} value={c.value}>
                  {t(`quickapp.choice.${c.value}`, { defaultValue: c.label })}
                </option>
              ))}
            </select>
          </label>
        ))}
      </div>

      <dl className="grid gap-3 sm:grid-cols-2">
        <Fact label={t("quickapp.folder")} value={`${overview.www}${sep}${finalName}`} />
        <Fact label={t("quickapp.site")} value={`${finalName}.${overview.tld}`} />
      </dl>

      {toInstall.length > 0 && (
        <p className="rounded-md bg-hover px-3 py-2 text-sm">
          {t("quickapp.willInstall", { list: toInstall.map((n) => t(`quickapp.need.${n}`)).join(", ") })}
        </p>
      )}

      <ErrorBox error={action.error} />
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="submit"
          disabled={!valid || taken || busy || action.isPending}
          className="flex items-center gap-1.5 rounded-md bg-accent px-4 py-2 text-sm font-medium text-on-accent outline-none hover:bg-accent-strong focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-50 [&_svg]:size-4"
        >
          <Rocket />
          {t("quickapp.create")}
        </button>
        {busy && <span className="text-sm text-muted">{t("quickapp.busy")}</span>}
      </div>
    </form>
  );
}

/** Output received through events since the last fetch of the job. */
interface Live {
  jobId: number;
  first: number;
  lines: string[];
  step: Step | null;
  download: [number, number | null] | null;
}

function useLiveOutput(jobId: number) {
  const [live, setLive] = useState<Live | null>(null);
  useEffect(() => {
    const unlisten = listen<OutputEvent>("quickapp-output", ({ payload }) => {
      if (payload.jobId !== jobId) return;
      setLive((prev) => {
        const base =
          prev && prev.jobId === jobId
            ? prev
            : { jobId, first: payload.first, lines: [], step: null, download: null };
        let lines = base.lines;
        let first = base.first;
        if (payload.lines.length > 0) {
          if (first + lines.length === payload.first) lines = [...lines, ...payload.lines];
          else ({ first, lines } = { first: payload.first, lines: payload.lines });
        }
        return {
          jobId,
          first,
          lines,
          step: payload.step ?? base.step,
          // A new step clears the progress bar of the previous download.
          download: payload.step ? payload.download : (payload.download ?? base.download),
        };
      });
    });
    return () => void unlisten.then((fn) => fn());
  }, [jobId]);
  return live;
}

function JobPanel({ job, overview }: { job: Job; overview: QuickAppOverview }) {
  const { t } = useTranslation();
  const action = useQuickAppAction();
  const live = useLiveOutput(job.id);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const running = job.state === "running";

  // The fetched lines, then whatever arrived through events after them.
  const lines = useMemo(() => {
    const end = job.dropped + job.lines.length;
    if (!live || live.first + live.lines.length <= end) return job.lines;
    return [...job.lines, ...live.lines.slice(Math.max(0, end - live.first))];
  }, [job, live]);
  const step = (running && live?.step) || job.step;
  const download = running ? (live ? live.download : job.download) : null;
  const percent = download?.[1] ? Math.min(100, Math.round((download[0] / download[1]) * 100)) : null;

  // A new job: bring its panel into view (the page may be scrolled to the form).
  const panelRef = useRef<HTMLElement>(null);
  useEffect(() => {
    panelRef.current?.scrollIntoView?.({ block: "start", behavior: "smooth" });
  }, [job.id]);

  const logRef = useRef<HTMLPreElement>(null);
  const stick = useRef(true);
  useEffect(() => {
    const el = logRef.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [lines.length]);

  const recipe = overview.recipes.find((r) => r.id === job.recipe);
  const usesDb = recipe?.needs.includes("database") ?? false;
  const db = overview.database;

  return (
    <section
      ref={panelRef}
      className="scroll-mt-4 space-y-3 rounded-xl border border-line p-5"
      aria-labelledby="quickapp-job-title"
      aria-busy={running}
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 id="quickapp-job-title" className="text-lg font-semibold">
            {t(`quickapp.state.${job.state}`, { name: job.name, recipe: job.recipeName })}
          </h2>
          {step && running && (
            <p className="text-sm text-muted" role="status">
              {step.total > 0 ? t("quickapp.step", { index: step.index, total: step.total, label: step.label }) : step.label}
            </p>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          {running && (
            <Button onClick={() => action.mutate(quickappCancel)} icon={<X />}>
              {t("actions.cancel")}
            </Button>
          )}
          {job.state === "succeeded" && (
            <>
              <Button primary onClick={() => action.mutate(() => siteOpen(job.domain))} icon={<ExternalLink />}>
                {t("quickapp.openSite")}
              </Button>
              <Button onClick={() => action.mutate(() => editorOpen({ kind: "site", name: job.name }))} icon={<Code />}>
                {t("quickapp.openEditor")}
              </Button>
              <Button onClick={() => action.mutate(() => siteOpenFolder(job.name))} icon={<FolderOpen />}>
                {t("quickapp.openFolder")}
              </Button>
            </>
          )}
          {!running && job.partial && !confirmDelete && (
            <Button onClick={() => setConfirmDelete(true)} icon={<Trash2 />}>
              {t("quickapp.deletePartial")}
            </Button>
          )}
          {!running && (
            <Button onClick={() => action.mutate(quickappDismiss)} aria-label={t("quickapp.dismiss")} title={t("quickapp.dismiss")} icon={<X />} />
          )}
        </div>
      </div>

      {confirmDelete && (
        <div role="alertdialog" aria-labelledby="quickapp-delete-q" className="flex flex-wrap items-center gap-3 rounded-md bg-danger-soft px-3 py-2 text-sm">
          <span id="quickapp-delete-q" className="text-danger">
            {job.createdDatabase
              ? t("quickapp.deleteConfirmDb", { dir: job.dir, db: job.createdDatabase })
              : t("quickapp.deleteConfirm", { dir: job.dir })}
          </span>
          <Button
            onClick={() => {
              setConfirmDelete(false);
              action.mutate(quickappDeletePartial);
            }}
            icon={<Trash2 />}
          >
            {t("quickapp.deleteYes")}
          </Button>
          <Button onClick={() => setConfirmDelete(false)}>{t("actions.cancel")}</Button>
        </div>
      )}

      {job.error && (
        <p role="alert" className="rounded-md bg-danger-soft px-3 py-2 font-mono text-xs text-danger" dir="ltr">
          {job.error}
        </p>
      )}
      {job.warning && (
        <p role="status" className="rounded-md bg-warn-soft px-3 py-2 text-sm text-warn">
          {t("quickapp.warning")} <span className="font-mono text-xs" dir="ltr">{job.warning}</span>
        </p>
      )}
      <ErrorBox error={action.error} />

      {download && (
        <div>
          <div className="h-1.5 overflow-hidden rounded-full bg-hover" aria-hidden="true">
            <div
              className={`h-full bg-accent transition-[width] duration-300 ease-out ${percent === null ? "w-full progress-flow" : ""}`}
              style={percent !== null ? { width: `${percent}%` } : undefined}
            />
          </div>
          <p className="mt-1 text-xs text-muted">
            {t("quickapp.downloaded", { done: formatBytes(download[0]), total: download[1] ? formatBytes(download[1]) : "?" })}
          </p>
        </div>
      )}

      {job.state === "succeeded" && (
        <dl className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <Fact label={t("quickapp.site")} value={job.url} />
          <Fact label={t("quickapp.folder")} value={job.dir} />
          {usesDb && (
            <>
              <Fact label={t("quickapp.database")} value={job.name.replace(/-/g, "_")} />
              <Fact
                label={t("quickapp.dbLogin")}
                value={`${db.user}@${db.host}:${db.port}${db.password ? "" : ` (${t("quickapp.noPassword")})`}`}
              />
            </>
          )}
        </dl>
      )}

      <pre
        ref={logRef}
        onScroll={(e) => {
          const el = e.currentTarget;
          stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
        className="max-h-80 min-h-32 overflow-auto rounded-lg bg-hover p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-words"
        dir="ltr"
        aria-label={t("quickapp.output")}
        tabIndex={0}
      >
        {lines.join("\n") || t("quickapp.waiting")}
      </pre>
      <p className="text-xs text-muted" dir="auto">
        {t("quickapp.logFile", { path: job.logFile })}
      </p>
    </section>
  );
}
