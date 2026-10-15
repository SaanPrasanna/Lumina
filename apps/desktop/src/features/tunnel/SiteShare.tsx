import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Loader2, Settings2, Share2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { ErrorBox } from "@/components/ErrorBox";
import { Button } from "@/components/ui";
import { PortConflictDialog, type PortConflictRequest } from "@/features/services/PortConflictDialog";
import { webConflictRequest } from "@/features/services/WebCard";
import type { WebSettings } from "@/features/settings/api";
import { isCommandError, portConflictOf } from "@/lib/ipc";
import { SETUP_ERRORS, TUNNEL_KEY, type TunnelOverview, tunnelStart } from "@/features/tunnel/api";
import { TunnelCard } from "@/features/tunnel/TunnelCard";

/**
 * "Share" for one site: opens a public address and shows it below the site's
 * row. Sharing starts the web server; if another program holds its port (e.g.
 * another local server on 80), the same dialog as on the Services page offers
 * 8080/8443 and shares again.
 */
export function useSiteShare(name: string, overview: TunnelOverview | undefined, web?: WebSettings) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [conflict, setConflict] = useState<PortConflictRequest | null>(null);
  const tunnel = overview?.tunnels.find((x) => x.name === name);
  // The saved reserved domain, unless another site already uses it.
  const domain = overview?.domain && !overview.tunnels.some((x) => x.publicUrl.endsWith(`//${overview.domain}`)) ? overview.domain : null;
  const start = useMutation({
    mutationFn: () => tunnelStart(name, domain),
    onSettled: () => queryClient.invalidateQueries({ queryKey: TUNNEL_KEY }),
    onError: (err) => {
      if (!web) return;
      const busy = portConflictOf(err, web.http_port) ?? portConflictOf(err, web.https_port);
      if (!busy) return;
      const request = webConflictRequest(busy, web);
      setConflict({
        ...request,
        apply: async (alternative) => {
          await request.apply(alternative);
          start.mutate();
        },
      });
    },
  });
  // Shown while opening, with the tunnel, or with the reason it did not open.
  const visible = start.isPending || (open && (!!tunnel || !!start.error));
  const toggle = () => {
    if (visible) {
      setOpen(false);
      return;
    }
    setOpen(true);
    if (!tunnel) start.mutate();
  };
  return { open: visible, tunnel, start, toggle, conflict, closeConflict: () => setConflict(null) };
}

export function ShareButton({ share, disabled }: { share: ReturnType<typeof useSiteShare>; disabled?: boolean }) {
  const { t } = useTranslation();
  return (
    <Button
      onClick={share.toggle}
      disabled={disabled || share.start.isPending}
      aria-expanded={share.open}
      icon={share.start.isPending ? <Loader2 className="animate-spin" /> : <Share2 />}
    >
      {share.tunnel ? t("tunnel.shared") : t("tunnel.share")}
    </Button>
  );
}

export function SharePanel({ share }: { share: ReturnType<typeof useSiteShare> }) {
  return (
    <>
      {share.conflict && <PortConflictDialog request={share.conflict} onClose={share.closeConflict} />}
      <ShareResult share={share} />
    </>
  );
}

function ShareResult({ share }: { share: ReturnType<typeof useSiteShare> }) {
  const { t } = useTranslation();
  if (!share.open) return null;
  if (share.start.isPending) {
    return (
      <p role="status" className="mt-3 flex items-center gap-2 text-sm text-muted">
        <Loader2 className="size-4 animate-spin" aria-hidden />
        {t("tunnel.opening")}
      </p>
    );
  }
  if (share.tunnel) {
    return (
      <div className="mt-3">
        <TunnelCard tunnel={share.tunnel} />
      </div>
    );
  }
  const error = share.start.error;
  if (!error) return null;
  const setup = isCommandError(error) && SETUP_ERRORS.includes(error.code);
  return (
    <div className="mt-3 space-y-2">
      <ErrorBox error={error} />
      {setup && (
        <Link
          to="/tunnel"
          className="inline-flex items-center gap-1.5 rounded-md border border-line px-3 py-1.5 text-sm hover:bg-hover [&_svg]:size-4"
        >
          <Settings2 aria-hidden />
          {t("tunnel.setUp")}
        </Link>
      )}
    </div>
  );
}
