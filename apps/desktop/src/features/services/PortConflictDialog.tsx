import { useMutation, useQueryClient } from "@tanstack/react-query";
import { TriangleAlert } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import type { PortConflict } from "@/lib/ipc";

/** 80 → 8080, 443 → 8443, anything else (3306, 6379, …) → the next port. */
export function alternativePort(port: number): number {
  if (port === 80) return 8080;
  if (port === 443) return 8443;
  return port + 1;
}

/** What the dialog offers to fix: built by the card whose service hit the conflict. */
export interface PortConflictRequest {
  conflict: PortConflict;
  /** Display name, e.g. "MySQL". */
  service: string;
  /** The web server's port also changes the sites' URLs, which the text mentions. */
  isWeb: boolean;
  /** Saves `alternative` as the service's port and starts it. */
  apply: (alternative: number) => Promise<unknown>;
}

/**
 * Explains which program holds a service's port and offers to move the service
 * to an alternative port (blueprint: IIS or another Laragon on 80, an existing
 * MySQL on 3306).
 */
export function PortConflictDialog({ request, onClose }: { request: PortConflictRequest; onClose: () => void }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const dialog = useRef<HTMLDialogElement>(null);
  const { conflict } = request;
  const alternative = alternativePort(conflict.port);
  const owner = conflict.owner ?? t("portConflict.unknownOwner");

  useEffect(() => {
    const d = dialog.current;
    if (d && !d.open) d.showModal?.();
  }, []);

  const move = useMutation({
    mutationFn: () => request.apply(alternative),
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: ["services"] }),
        queryClient.invalidateQueries({ queryKey: ["settings"] }),
      ]),
    onSuccess: onClose,
  });

  return (
    <dialog
      ref={dialog}
      aria-labelledby="port-conflict-title"
      onCancel={onClose}
      className="m-auto w-full max-w-md rounded-xl border border-line bg-surface p-6 text-fg backdrop:bg-black/50"
    >
      <h2 id="port-conflict-title" className="flex items-center gap-2 text-lg font-semibold">
        <TriangleAlert className="size-5 text-warn" aria-hidden />
        {t("portConflict.title", { port: conflict.port })}
      </h2>
      <p className="mt-3 text-sm text-fg-soft">
        {request.isWeb
          ? t("portConflict.body", { owner, port: conflict.port, alternative })
          : t("portConflict.bodyService", { owner, port: conflict.port, service: request.service, alternative })}
      </p>
      <div className="mt-4">
        <ErrorBox error={move.error} />
      </div>
      <div className="mt-5 flex justify-end gap-2">
        <button type="button" onClick={onClose} className="rounded-md border border-line px-4 py-2 text-sm hover:bg-hover">
          {t("actions.cancel")}
        </button>
        <button
          type="button"
          onClick={() => move.mutate()}
          disabled={move.isPending}
          className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-on-accent hover:bg-accent-strong disabled:opacity-60"
        >
          {t("portConflict.use", { alternative })}
        </button>
      </div>
    </dialog>
  );
}
