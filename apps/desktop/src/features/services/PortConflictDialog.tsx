import { useMutation, useQueryClient } from "@tanstack/react-query";
import { TriangleAlert } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { serviceStart, webKind } from "@/features/services/api";
import { settingsUpdate, type WebSettings } from "@/features/settings/api";
import type { PortConflict } from "@/lib/ipc";

/** 80 → 8080, 443 → 8443, anything else → the next port. */
export function alternativePort(port: number): number {
  if (port === 80) return 8080;
  if (port === 443) return 8443;
  return port + 1;
}

/**
 * Explains which program holds the web port and offers to move Lumina to an
 * alternative port (blueprint: IIS, Skype or another Laragon on port 80).
 */
export function PortConflictDialog(props: {
  conflict: PortConflict;
  web: WebSettings;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const dialog = useRef<HTMLDialogElement>(null);
  const alternative = alternativePort(props.conflict.port);

  useEffect(() => {
    const d = dialog.current;
    if (d && !d.open) d.showModal?.();
  }, []);

  const move = useMutation({
    mutationFn: async () => {
      await settingsUpdate("web", {
        ...props.web,
        http_port: alternative,
        https_port: props.web.https_port === 443 ? 8443 : props.web.https_port,
      });
      await serviceStart(webKind(props.web.server));
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["services"] }),
    onSuccess: props.onClose,
  });

  return (
    <dialog
      ref={dialog}
      aria-labelledby="port-conflict-title"
      onCancel={props.onClose}
      className="m-auto w-full max-w-md rounded-xl border border-line bg-surface p-6 text-fg backdrop:bg-black/50"
    >
      <h2 id="port-conflict-title" className="flex items-center gap-2 text-lg font-semibold">
        <TriangleAlert className="size-5 text-warn" aria-hidden />
        {t("portConflict.title", { port: props.conflict.port })}
      </h2>
      <p className="mt-3 text-sm text-fg-soft">
        {t("portConflict.body", {
          owner: props.conflict.owner ?? t("portConflict.unknownOwner"),
          port: props.conflict.port,
          alternative,
        })}
      </p>
      <div className="mt-4">
        <ErrorBox error={move.error} />
      </div>
      <div className="mt-5 flex justify-end gap-2">
        <button type="button" onClick={props.onClose} className="rounded-md border border-line px-4 py-2 text-sm hover:bg-hover">
          {t("actions.cancel")}
        </button>
        <button
          type="button"
          onClick={() => move.mutate()}
          disabled={move.isPending}
          className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent-strong disabled:opacity-60"
        >
          {t("portConflict.use", { alternative })}
        </button>
      </div>
    </dialog>
  );
}
