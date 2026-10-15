import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { type FormEvent, useState } from "react";
import { useTranslation } from "react-i18next";
import { LanguageSelect } from "@/components/LanguageSelect";
import { Logo } from "@/components/Logo";
import { BOOTSTRAP_QUERY_KEY, type BootstrapStatus, bootstrapComplete } from "@/features/onboarding/api";
import { isCommandError } from "@/lib/ipc";

/** First-run wizard: pick the data root and the UI language. */
export function FirstRunPage({ status }: { status: BootstrapStatus }) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const [dataRoot, setDataRoot] = useState(status.defaultDataRoot);

  const complete = useMutation({
    mutationFn: () => bootstrapComplete(dataRoot.trim(), i18n.resolvedLanguage ?? "en"),
    onSuccess: (next) => queryClient.setQueryData(BOOTSTRAP_QUERY_KEY, next),
  });

  const browse = async () => {
    const picked = await open({ directory: true, defaultPath: dataRoot });
    if (typeof picked === "string") setDataRoot(picked);
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    complete.mutate();
  };

  const error = complete.error;
  const errorText = error
    ? isCommandError(error)
      ? t(`errors.${error.code}`, { defaultValue: t("errors.unknown") })
      : t("errors.unknown")
    : null;

  return (
    <div className="flex h-full items-center justify-center overflow-y-auto bg-surface p-6 text-fg">
      <form onSubmit={submit} className="w-full max-w-md space-y-6">
        <div className="flex flex-col items-center gap-3 text-center">
          <Logo className="size-16" />
          <h1 className="text-2xl font-semibold">{t("firstRun.title")}</h1>
          <p className="text-sm text-muted">{t("firstRun.subtitle")}</p>
        </div>

        <div className="space-y-1.5">
          <label htmlFor="data-root" className="text-sm font-medium">
            {t("firstRun.dataRoot")}
          </label>
          <div className="flex gap-2">
            <input
              id="data-root"
              dir="ltr"
              className="min-w-0 flex-1 rounded-md border border-line bg-surface px-3 py-2 font-mono text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent"
              value={dataRoot}
              onChange={(e) => setDataRoot(e.target.value)}
              spellCheck={false}
              required
            />
            <button
              type="button"
              onClick={() => void browse()}
              className="flex items-center gap-1.5 rounded-md border border-line px-3 text-sm hover:bg-hover"
            >
              <FolderOpen className="size-4" aria-hidden />
              {t("firstRun.browse")}
            </button>
          </div>
          <p className="text-xs text-muted">{t("firstRun.dataRootHint")}</p>
        </div>

        <LanguageSelect />

        {errorText && (
          <p role="alert" className="rounded-md bg-danger-soft px-3 py-2 text-sm text-danger">
            {errorText}
            {isCommandError(error) && (
              <span className="mt-1 block font-mono text-xs opacity-80" dir="ltr">
                {error.message}
              </span>
            )}
          </p>
        )}

        <button
          type="submit"
          disabled={complete.isPending || dataRoot.trim() === ""}
          className="w-full rounded-md bg-accent py-2.5 text-sm font-medium text-on-accent hover:bg-accent-strong disabled:opacity-60"
        >
          {complete.isPending ? t("firstRun.saving") : t("firstRun.continue")}
        </button>
      </form>
    </div>
  );
}
