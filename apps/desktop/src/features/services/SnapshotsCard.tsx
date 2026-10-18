import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Camera, RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { Button } from "@/components/ui";
import { formatBytes } from "@/features/packages/api";
import { SNAPSHOTS_KEY, type Snapshot, snapshotCreate, snapshotDatabases, snapshotDelete, snapshotRestore, snapshotsList } from "@/features/services/api";

/** Save a database to a file and restore it later (before a migration, between test runs). */
export function SnapshotsCard({ engines }: { engines: string[] }) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const snapshots = useQuery({ queryKey: SNAPSHOTS_KEY, queryFn: snapshotsList });
  const action = useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () => queryClient.invalidateQueries({ queryKey: SNAPSHOTS_KEY }),
  });
  const [confirm, setConfirm] = useState<string | null>(null);
  const list = snapshots.data ?? [];
  if (engines.length === 0 && list.length === 0) return null;

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="snapshots-title">
      <h2 id="snapshots-title" className="text-lg font-semibold">
        {t("snapshots.title")}
      </h2>
      <p className="mt-1 text-sm text-muted">{t("snapshots.description")}</p>
      <div className="mt-4 space-y-3">
        {engines.map((engine) => (
          <TakeSnapshot key={engine} engine={engine} busy={action.isPending} onTake={(db) => action.mutate(() => snapshotCreate(engine, db))} />
        ))}
      </div>
      <div className="mt-3">
        <ErrorBox error={action.error} />
      </div>
      {list.length > 0 && (
        <ul className="mt-3 divide-y divide-line rounded-md border border-line">
          {list.map((s: Snapshot) => (
            <li key={s.id} className="flex flex-wrap items-center justify-between gap-3 px-3 py-2 text-sm">
              <span className="min-w-0">
                <span className="font-mono">{s.database}</span>{" "}
                <span className="text-muted">
                  {t("snapshots.meta", { engine: s.engine, date: new Date(s.created * 1000).toLocaleString(i18n.language), size: formatBytes(s.size) })}
                </span>
              </span>
              <span className="flex gap-2">
                <Button
                  onClick={() => {
                    if (confirm !== s.id) return setConfirm(s.id);
                    setConfirm(null);
                    action.mutate(() => snapshotRestore(s.id));
                  }}
                  disabled={action.isPending || !engines.includes(s.engine)}
                  title={engines.includes(s.engine) ? undefined : t("snapshots.startServer", { engine: s.engine })}
                  icon={<RotateCcw />}
                >
                  {confirm === s.id ? t("snapshots.confirmRestore", { database: s.database }) : t("snapshots.restore")}
                </Button>
                <Button onClick={() => action.mutate(() => snapshotDelete(s.id))} disabled={action.isPending} icon={<Trash2 />} aria-label={t("snapshots.delete")} title={t("snapshots.delete")} />
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function TakeSnapshot({ engine, busy, onTake }: { engine: string; busy: boolean; onTake: (db: string) => void }) {
  const { t } = useTranslation();
  const dbs = useQuery({ queryKey: [...SNAPSHOTS_KEY, "databases", engine], queryFn: () => snapshotDatabases(engine) });
  const [db, setDb] = useState("");
  const chosen = db || dbs.data?.[0] || "";
  return (
    <div className="flex flex-wrap items-end gap-3">
      <label className="flex flex-col gap-1.5 text-sm">
        <span className="font-medium">{t("snapshots.database", { engine })}</span>
        <select className="min-w-48 rounded-md border border-line bg-surface px-3 py-1.5" value={chosen} onChange={(e) => setDb(e.target.value)} disabled={busy || !dbs.data?.length}>
          {(dbs.data ?? []).map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      </label>
      <Button onClick={() => chosen && onTake(chosen)} disabled={busy || !chosen} icon={<Camera />}>
        {t("snapshots.take")}
      </Button>
      {dbs.isSuccess && dbs.data.length === 0 && <p className="pb-1.5 text-sm text-muted">{t("snapshots.noDatabases")}</p>}
      {dbs.error ? <ErrorBox error={dbs.error} /> : null}
    </div>
  );
}
