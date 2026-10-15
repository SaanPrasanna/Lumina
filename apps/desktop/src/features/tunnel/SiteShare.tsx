import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Loader2, Settings2, Share2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { ErrorBox } from "@/components/ErrorBox";
import { Button } from "@/components/ui";
import { isCommandError } from "@/lib/ipc";
import { SETUP_ERRORS, TUNNEL_KEY, type TunnelOverview, tunnelStart } from "@/features/tunnel/api";
import { TunnelCard } from "@/features/tunnel/TunnelCard";

/** "Share" for one site: opens a public address and shows it below the site's row. */
export function useSiteShare(name: string, overview: TunnelOverview | undefined) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const tunnel = overview?.tunnels.find((x) => x.name === name);
  // The saved reserved domain, unless another site already uses it.
  const domain = overview?.domain && !overview.tunnels.some((x) => x.publicUrl.endsWith(`//${overview.domain}`)) ? overview.domain : null;
  const start = useMutation({
    mutationFn: () => tunnelStart(name, domain),
    onSettled: () => queryClient.invalidateQueries({ queryKey: TUNNEL_KEY }),
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
  return { open: visible, tunnel, start, toggle };
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
