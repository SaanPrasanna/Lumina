import { useQuery } from "@tanstack/react-query";
import { Layers } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { PROFILES_KEY, profilesList, profileSwitch } from "@/features/profiles/api";
import { useProfileAction } from "@/features/profiles";

/** Sidebar header: the current profile, switchable in one click. */
export function ProfileSwitcher() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const list = useQuery({ queryKey: PROFILES_KEY, queryFn: profilesList });
  const action = useProfileAction();
  const profiles = list.data?.profiles ?? [];
  if (profiles.length === 0) return null;
  const current = profiles.find((p) => p.current)?.name ?? "";

  return (
    <label className="mx-3 mb-2 flex items-center gap-2 rounded-md border border-line px-2 py-1.5 text-sm">
      <Layers className="size-4 shrink-0 text-muted" aria-hidden />
      <span className="sr-only">{t("profiles.switcher")}</span>
      <select
        className="min-w-0 flex-1 bg-transparent outline-none"
        value={current}
        disabled={action.isPending}
        onChange={(e) =>
          action.mutate(() => profileSwitch(e.target.value), {
            // Errors are shown on the Profiles page.
            onError: () => navigate("/profiles"),
          })
        }
      >
        {current === "" && <option value="">{t("profiles.custom")}</option>}
        {profiles.map((p) => (
          <option key={p.name} value={p.name} disabled={p.missing.length > 0}>
            {p.name}
          </option>
        ))}
      </select>
      {action.isPending && <span className="size-2 shrink-0 animate-pulse rounded-full bg-accent" aria-label={t("profiles.switching")} />}
    </label>
  );
}
