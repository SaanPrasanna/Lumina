import { useMutation, useQuery } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDownToLine, Eraser, ExternalLink, FolderOpen } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui";
import {
  appendText,
  type Level,
  levelOf,
  LOGS_KEY,
  type LogSource,
  logsList,
  logsOpen,
  logsOpenFolder,
  logsRead,
} from "@/features/logs/api";

/** Lines kept in view; older ones are dropped (the file keeps them). */
const MAX_LINES = 20_000;
const POLL_MS = 1000;
const ROW_PX = 20;

type LevelFilter = "all" | "warn" | "error";

/** Text colour per line severity. */
const TONE: Record<Level, string> = { error: "text-danger", warn: "text-warn", info: "" };

/** Logs: every log Lumina writes, as a live tail with a filter. */
export function LogsPage() {
  const { t } = useTranslation();
  const sources = useQuery({ queryKey: LOGS_KEY, queryFn: logsList, refetchInterval: 5000 });
  const [selected, setSelected] = useState<string | null>(null);
  const list = sources.data ?? [];
  // The app log first; once chosen, a log stays selected even while it is missing.
  const current = selected ?? list[0]?.id ?? null;
  const action = useMutation({ mutationFn: (fn: () => Promise<unknown>) => fn() });

  return (
    <>
      <PageHeader
        title={t("nav.logs")}
        actions={
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => current && action.mutate(() => logsOpen(current))} disabled={!current} icon={<ExternalLink />}>
              {t("logs.openFile")}
            </Button>
            <Button onClick={() => action.mutate(logsOpenFolder)} icon={<FolderOpen />}>
              {t("logs.openFolder")}
            </Button>
          </div>
        }
      />
      <div className="flex min-h-0 flex-col gap-3 px-8 pb-8">
        <ErrorBox error={sources.error ?? action.error} />
        {sources.data && list.length === 0 && <p className="text-sm text-muted">{t("logs.none")}</p>}
        {current && <LogView key={current} id={current} sources={list} onSelect={setSelected} />}
      </div>
    </>
  );
}

function SourceSelect({ sources, value, onChange }: { sources: LogSource[]; value: string; onChange: (id: string) => void }) {
  const { t } = useTranslation();
  const groups: [LogSource["group"], string][] = [
    ["app", t("logs.group.app")],
    ["service", t("logs.group.service")],
    ["quickapp", t("logs.group.quickapp")],
  ];
  return (
    <label className="flex min-w-56 flex-col gap-1.5 text-sm">
      <span className="font-medium">{t("logs.source")}</span>
      <select className="rounded-md border border-line bg-surface px-3 py-1.5" value={value} onChange={(e) => onChange(e.target.value)}>
        {groups.map(([group, label]) => {
          const items = sources.filter((s) => s.group === group);
          return items.length === 0 ? null : (
            <optgroup key={group} label={label}>
              {items.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.id === "app" ? t("logs.appLog") : s.id.replace(/^quickapp\//, "")}
                </option>
              ))}
            </optgroup>
          );
        })}
      </select>
    </label>
  );
}

/** One log, followed live. */
function LogView({ id, sources, onSelect }: { id: string; sources: LogSource[]; onSelect: (id: string) => void }) {
  const { t } = useTranslation();
  const [lines, setLines] = useState<string[]>([]);
  const [error, setError] = useState<unknown>(null);
  const [skipped, setSkipped] = useState(false);
  const [filter, setFilter] = useState("");
  const [level, setLevel] = useState<LevelFilter>("all");
  const [follow, setFollow] = useState(true);
  const state = useRef({ offset: null as number | null, pending: "", lines: [] as string[] });

  // Poll for new text; each read continues where the last one stopped.
  useEffect(() => {
    let stopped = false;
    let timer = 0;
    const tick = async () => {
      try {
        const chunk = await logsRead(id, state.current.offset);
        if (stopped) return;
        setError(null);
        const first = state.current.offset === null;
        state.current.offset = chunk.offset;
        if (chunk.reset) state.current.pending = "";
        if (first || chunk.reset) setSkipped(chunk.skipped);
        if (chunk.text || chunk.reset) {
          // Computed here, not in a state updater: updaters may run twice.
          const next = chunk.reset ? [] : state.current.lines.slice();
          state.current.pending = appendText(next, state.current.pending, chunk.text);
          state.current.lines = next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
          setLines(state.current.lines);
        }
      } catch (err) {
        if (!stopped) setError(err);
      }
      if (!stopped) timer = window.setTimeout(() => void tick(), POLL_MS);
    };
    void tick();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  }, [id]);

  const shown = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    return lines.filter((line) => {
      if (needle && !line.toLowerCase().includes(needle)) return false;
      if (level === "all") return true;
      const l = levelOf(line);
      return level === "error" ? l === "error" : l !== "info";
    });
  }, [lines, filter, level]);

  const scroller = useRef<HTMLDivElement>(null);
  // The React Compiler skips this component (TanStack Virtual is not
  // memoization-safe); nothing here relies on it.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: shown.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW_PX,
    overscan: 20,
    initialRect: { width: 800, height: 480 },
  });

  useEffect(() => {
    if (follow && shown.length > 0) virtualizer.scrollToIndex(shown.length - 1, { align: "end" });
  }, [follow, shown.length, virtualizer]);

  // Scrolling up pauses following; scrolling back to the bottom resumes it.
  const onScroll = () => {
    const el = scroller.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < ROW_PX * 2;
    if (atBottom !== follow) setFollow(atBottom);
  };

  const source = sources.find((s) => s.id === id);

  return (
    <>
      <div className="flex flex-wrap items-end gap-3">
        <SourceSelect sources={sources} value={id} onChange={onSelect} />
        <label className="flex min-w-48 flex-1 flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("logs.filter")}</span>
          <input
            type="search"
            className="rounded-md border border-line bg-surface px-3 py-1.5"
            placeholder={t("logs.filterPlaceholder")}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </label>
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("logs.level")}</span>
          <select className="rounded-md border border-line bg-surface px-3 py-1.5" value={level} onChange={(e) => setLevel(e.target.value as LevelFilter)}>
            <option value="all">{t("logs.levels.all")}</option>
            <option value="warn">{t("logs.levels.warn")}</option>
            <option value="error">{t("logs.levels.error")}</option>
          </select>
        </label>
        <Button onClick={() => setFollow(true)} aria-pressed={follow} icon={<ArrowDownToLine />}>
          {t("logs.follow")}
        </Button>
        <Button
          onClick={() => {
            state.current.lines = [];
            setLines([]);
          }}
          icon={<Eraser />}
        >
          {t("logs.clear")}
        </Button>
      </div>
      {source && (
        <p className="truncate font-mono text-xs text-muted" dir="ltr">
          {source.path}
        </p>
      )}
      <ErrorBox error={error} />
      {skipped && <p className="text-xs text-muted">{t("logs.skipped")}</p>}
      <div
        ref={scroller}
        onScroll={onScroll}
        role="log"
        aria-label={t("logs.output")}
        aria-live="off"
        tabIndex={0}
        className="h-[28rem] overflow-auto rounded-lg border border-line bg-sidebar font-mono text-xs outline-none [contain:strict] focus-visible:ring-2 focus-visible:ring-accent"
        dir="ltr"
      >
        {shown.length === 0 ? (
          <p className="p-4 text-muted">{lines.length === 0 ? t("logs.empty") : t("logs.noMatch")}</p>
        ) : (
          <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map((row) => {
              const line = shown[row.index];
              const tone = TONE[levelOf(line)];
              return (
                <div
                  key={row.key}
                  className={`absolute start-0 top-0 whitespace-pre px-3 ${tone}`}
                  style={{ height: ROW_PX, lineHeight: `${ROW_PX}px`, transform: `translateY(${row.start}px)` }}
                >
                  {line || " "}
                </div>
              );
            })}
          </div>
        )}
      </div>
      <p className="text-xs text-muted">{t("logs.count", { shown: shown.length, total: lines.length })}</p>
    </>
  );
}
