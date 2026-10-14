import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, Minus, Plus, ShieldCheck, SquareTerminal, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui";
import { PATH_KEY, pathAdd, pathAddSystem, pathRemove, pathStatus, terminalOpen } from "@/features/env-path/api";

/** Tools › PATH: put Lumina's PHP, Composer, Node.js, ... on the terminal's PATH. */
export function EnvPathPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const status = useQuery({ queryKey: PATH_KEY, queryFn: pathStatus });
  const action = useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSuccess: (data) => {
      if (data && typeof data === "object" && "shimDir" in data) queryClient.setQueryData(PATH_KEY, data);
    },
  });
  const data = status.data;
  const busy = action.isPending;

  return (
    <>
      <PageHeader
        title={t("nav.envPath")}
        actions={
          <Button onClick={() => action.mutate(() => terminalOpen())} disabled={busy} icon={<SquareTerminal />}>
            {t("path.openTerminal")}
          </Button>
        }
      />
      <div className="space-y-4 px-8 pb-8">
        <p className="-mt-2 text-sm text-muted">{t("path.intro")}</p>
        <ErrorBox error={status.error ?? action.error} />

        {data && (
          <>
            <section className="rounded-xl border border-line p-5" aria-labelledby="path-status">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="flex items-center gap-2.5">
                  {data.added ? (
                    <CheckCircle2 className="size-5 text-ok" aria-hidden />
                  ) : (
                    <Minus className="size-5 text-muted" aria-hidden />
                  )}
                  <h2 id="path-status" className="font-semibold">
                    {data.added ? t("path.added") : t("path.notAdded")}
                  </h2>
                </div>
                {data.added ? (
                  <Button onClick={() => action.mutate(pathRemove)} disabled={busy} icon={<Minus />}>
                    {t("path.remove")}
                  </Button>
                ) : (
                  <Button primary onClick={() => action.mutate(pathAdd)} disabled={busy} icon={<Plus />}>
                    {t("path.add")}
                  </Button>
                )}
              </div>
              <p className="mt-2 font-mono text-xs text-muted" dir="ltr">
                {data.shimDir}
              </p>
              {data.files.length > 0 && (
                <p className="mt-1 text-xs text-muted">{t("path.files", { files: data.files.join(", "), interpolation: { escapeValue: false } })}</p>
              )}
              {action.isSuccess && <p className="mt-3 text-sm text-warn">{t("path.restartHint")}</p>}
              {data.addedSystem && <p className="mt-1 text-xs text-muted">{t("path.addedSystem")}</p>}
              {data.shadowed.length > 0 && !data.addedSystem && (
                <div className="mt-3 flex flex-wrap items-center justify-between gap-3 rounded-md border border-warn/40 bg-warn-soft px-4 py-3 text-sm">
                  <span>{t("path.systemHint")}</span>
                  <Button onClick={() => action.mutate(pathAddSystem)} disabled={busy} icon={<ShieldCheck />}>
                    {t("path.addSystem")}
                  </Button>
                </div>
              )}
              {data.shadowed.map((s) => (
                <p key={s.command} className="mt-3 flex items-start gap-2 text-sm text-warn">
                  <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
                  <span>
                    {t("path.shadowed", { command: s.command, found: s.found, interpolation: { escapeValue: false } })}
                  </span>
                </p>
              ))}
            </section>

            <section aria-labelledby="path-commands">
              <h2 id="path-commands" className="mb-2 text-sm font-semibold">
                {t("path.commands")}
              </h2>
              {data.shims.length === 0 ? (
                <p className="rounded-xl border border-dashed border-line p-5 text-sm text-muted">{t("path.noCommands")}</p>
              ) : (
                <ul className="divide-y divide-line rounded-xl border border-line">
                  {data.shims.map((s) => (
                    <li key={s.name} className="flex flex-wrap items-baseline gap-x-4 gap-y-1 px-4 py-2.5 text-sm">
                      <code className="w-24 shrink-0 font-mono font-medium">{s.name}</code>
                      <span>{s.describes}</span>
                      <span className="min-w-0 truncate font-mono text-xs text-muted" dir="ltr">
                        {s.program}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
              {!data.nativeShims && <p className="mt-2 text-xs text-muted">{t("path.cmdShims")}</p>}
            </section>
          </>
        )}
      </div>
    </>
  );
}
