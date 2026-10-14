import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { Check, Layers, Pencil, Plus, Save, Trash2 } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button, VersionSelect } from "@/components/ui";
import {
  describe,
  type Profile,
  PROFILES_KEY,
  type ProfileView,
  profileDelete,
  profileSave,
  profileSaveCurrent,
  profilesList,
  profileSwitch,
} from "@/features/profiles/api";
import {
  type DatabaseEngine,
  dbVersions,
  dbVersionsKey,
  PHP_VERSIONS_KEY,
  phpVersions,
  type WebServer,
  webVersions,
  webVersionsKey,
} from "@/features/services/api";

/** Everything that shows stack state. */
const AFFECTED = [PROFILES_KEY, ["services"], ["settings"], ["sites"], ["packages"]] as const;

export function useProfileAction() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () => Promise.all(AFFECTED.map((queryKey) => queryClient.invalidateQueries({ queryKey }))),
  });
}

/** Saved stacks: switch, edit, delete, and save the current stack. */
export function ProfilesPage() {
  const { t } = useTranslation();
  const list = useQuery({ queryKey: PROFILES_KEY, queryFn: profilesList });
  const action = useProfileAction();
  const [editing, setEditing] = useState<string | null>(null);
  const [trayError, setTrayError] = useState<string | null>(null);

  // A switch from the tray reports its failure here.
  useEffect(() => {
    const unlisten = listen<string>("profile-switch-failed", (e) => setTrayError(e.payload));
    return () => void unlisten.then((fn) => fn());
  }, []);

  const data = list.data;
  const switching = action.isPending;

  return (
    <>
      <PageHeader title={t("nav.profiles")} />
      <div className="space-y-4 px-8 pb-8">
        <p className="-mt-2 text-sm text-muted">{t("profiles.intro")}</p>
        <ErrorBox error={list.error ?? action.error ?? (trayError ? { code: "service", message: trayError } : null)} />
        {switching && (
          <p role="status" className="text-sm text-muted">
            {t("profiles.switching")}
          </p>
        )}

        {data && (
          <>
            <ul className="space-y-3" aria-label={t("nav.profiles")}>
              {data.profiles.map((p) =>
                editing === p.name ? (
                  <li key={p.name}>
                    <ProfileForm initial={p} original={p.name} onDone={() => setEditing(null)} />
                  </li>
                ) : (
                  <ProfileCard
                    key={p.name}
                    profile={p}
                    busy={switching}
                    onSwitch={() => {
                      setTrayError(null);
                      action.mutate(() => profileSwitch(p.name));
                    }}
                    onEdit={() => setEditing(p.name)}
                    onDelete={() => action.mutate(() => profileDelete(p.name))}
                  />
                ),
              )}
            </ul>
            {data.profiles.length === 0 && <p className="rounded-xl border border-dashed border-line p-6 text-sm text-muted">{t("profiles.empty")}</p>}

            {editing === "" ? (
              <ProfileForm initial={{ ...data.stack, name: "" }} original={null} onDone={() => setEditing(null)} />
            ) : (
              <SaveCurrent stack={data.stack} onNew={() => setEditing("")} />
            )}
          </>
        )}
      </div>
    </>
  );
}

function ProfileCard(props: {
  profile: ProfileView;
  busy: boolean;
  onSwitch: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const { profile } = props;
  const [confirm, setConfirm] = useState(false);

  return (
    <li className={`rounded-xl border p-4 ${profile.current ? "border-accent" : "border-line"}`}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Layers className="size-4 text-muted" aria-hidden />
            <span className="font-medium">{profile.name}</span>
            {profile.current && (
              <span className="flex items-center gap-1 rounded-full bg-accent-soft px-2 py-0.5 text-xs font-medium text-accent">
                <Check className="size-3" /> {t("profiles.current")}
              </span>
            )}
          </div>
          <p className="mt-1 text-sm text-muted" dir="ltr">
            {describe(profile, t("profiles.newest"))}
          </p>
          {profile.missing.length > 0 && (
            <p className="mt-1 text-sm text-warn">
              {t("profiles.missing", { what: profile.missing.join(", ") })}{" "}
              <Link to="/packages" className="underline">
                {t("profiles.openQuickAdd")}
              </Link>
            </p>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          {!profile.current && (
            <Button primary onClick={props.onSwitch} disabled={props.busy || profile.missing.length > 0}>
              {t("profiles.switch")}
            </Button>
          )}
          <Button onClick={props.onEdit} disabled={props.busy} icon={<Pencil />}>
            {t("sites.edit")}
          </Button>
          {confirm ? (
            <Button onClick={props.onDelete} disabled={props.busy} icon={<Trash2 />}>
              {t("profiles.confirmDelete")}
            </Button>
          ) : (
            <Button onClick={() => setConfirm(true)} disabled={props.busy} icon={<Trash2 />}>
              {t("profiles.delete")}
            </Button>
          )}
        </div>
      </div>
    </li>
  );
}

function SaveCurrent({ stack, onNew }: { stack: Profile; onNew: () => void }) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const action = useProfileAction();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    action.mutate(() => profileSaveCurrent(name), { onSuccess: () => setName("") });
  };
  return (
    <section className="rounded-xl border border-line p-4" aria-labelledby="save-current">
      <h2 id="save-current" className="font-medium">
        {t("profiles.saveCurrentTitle")}
      </h2>
      <p className="mt-1 text-sm text-muted" dir="ltr">
        {describe(stack, t("profiles.newest"))}
      </p>
      <form onSubmit={submit} className="mt-3 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("profiles.name")}</span>
          <input
            className="w-56 rounded-md border border-line bg-surface px-3 py-1.5"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("profiles.namePlaceholder")}
          />
        </label>
        <Button type="submit" primary disabled={action.isPending || name.trim() === ""} icon={<Save />}>
          {t("profiles.saveCurrent")}
        </Button>
        <Button onClick={onNew} icon={<Plus />}>
          {t("profiles.new")}
        </Button>
      </form>
      <div className="mt-2">
        <ErrorBox error={action.error} />
      </div>
    </section>
  );
}

/** Create or edit a profile from the installed versions. */
function ProfileForm({ initial, original, onDone }: { initial: Profile; original: string | null; onDone: () => void }) {
  const { t } = useTranslation();
  const [form, setForm] = useState<Profile>(initial);
  const action = useProfileAction();
  const php = useQuery({ queryKey: PHP_VERSIONS_KEY, queryFn: phpVersions });
  const web = useQuery({ queryKey: webVersionsKey(form.web), queryFn: () => webVersions(form.web) });
  const db = useQuery({
    queryKey: dbVersionsKey(form.database.engine),
    queryFn: () => dbVersions(form.database.engine),
  });
  const newest = (list: { version: string }[] | undefined) =>
    t("php.newest", { version: list?.[0]?.version ?? "—" });

  const submit = (e: FormEvent) => {
    e.preventDefault();
    action.mutate(() => profileSave(form, original), { onSuccess: onDone });
  };

  return (
    <form onSubmit={submit} className="space-y-4 rounded-xl border border-accent p-4" aria-label={t("profiles.editTitle")}>
      <label className="flex flex-col gap-1.5 text-sm">
        <span className="font-medium">{t("profiles.name")}</span>
        <input
          className="w-64 rounded-md border border-line bg-surface px-3 py-1.5"
          value={form.name}
          onChange={(e) => setForm({ ...form, name: e.target.value })}
          required
        />
      </label>
      <div className="grid gap-4 sm:grid-cols-2">
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("web.server")}</span>
          <select
            className="rounded-md border border-line bg-surface px-3 py-2"
            value={form.web}
            onChange={(e) => setForm({ ...form, web: e.target.value as WebServer, web_version: "" })}
          >
            <option value="nginx">nginx</option>
            <option value="apache">Apache</option>
          </select>
        </label>
        <VersionSelect
          label={t("web.version")}
          newestLabel={newest(web.data)}
          versions={(web.data ?? []).map((v) => v.version)}
          value={form.web_version}
          onChange={(v) => setForm({ ...form, web_version: v })}
        />
        <VersionSelect
          label="PHP"
          newestLabel={newest(php.data)}
          versions={(php.data ?? []).map((v) => v.version)}
          value={form.php}
          onChange={(v) => setForm({ ...form, php: v })}
        />
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="font-medium">{t("db.engine")}</span>
          <select
            className="rounded-md border border-line bg-surface px-3 py-2"
            value={form.database.engine}
            onChange={(e) => setForm({ ...form, database: { engine: e.target.value as DatabaseEngine, version: "" } })}
          >
            <option value="mysql">MySQL</option>
            <option value="mariadb">MariaDB</option>
          </select>
        </label>
        <VersionSelect
          label={t("db.version")}
          newestLabel={newest(db.data)}
          versions={(db.data ?? []).map((v) => v.version)}
          value={form.database.version}
          onChange={(v) => setForm({ ...form, database: { ...form.database, version: v } })}
        />
        <label className="flex items-center gap-2.5 self-end pb-2 text-sm">
          <input
            type="checkbox"
            className="size-4 accent-accent"
            checked={form.redis}
            onChange={(e) => setForm({ ...form, redis: e.target.checked })}
          />
          Redis
        </label>
      </div>
      <ErrorBox error={action.error} />
      <div className="flex gap-2">
        <Button type="submit" primary disabled={action.isPending} icon={<Save />}>
          {t("actions.save")}
        </Button>
        <Button onClick={onDone} disabled={action.isPending}>
          {t("actions.cancel")}
        </Button>
      </div>
    </form>
  );
}
