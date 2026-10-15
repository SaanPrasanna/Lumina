import { useMutation, useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Check, Copy, ExternalLink, Square } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { Button } from "@/components/ui";
import { qrSource, TUNNEL_KEY, type TunnelView, tunnelStop } from "@/features/tunnel/api";

/** An open tunnel: the public address with Copy / Open / Stop and a QR code for phones. */
export function TunnelCard({ tunnel }: { tunnel: TunnelView }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [copied, setCopied] = useState(false);
  const action = useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSettled: () => queryClient.invalidateQueries({ queryKey: TUNNEL_KEY }),
  });
  const copy = async () => {
    await navigator.clipboard.writeText(tunnel.publicUrl);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="flex flex-wrap items-start gap-5 rounded-lg border border-line p-4" aria-label={t("tunnel.sharedSite", { host: tunnel.host })}>
      {tunnel.qrSvg && (
        <img
          src={qrSource(tunnel.qrSvg)}
          alt={t("tunnel.qr", { url: tunnel.publicUrl })}
          className="size-36 shrink-0 rounded-md bg-white p-1"
        />
      )}
      <div className="min-w-0 flex-1 space-y-2">
        <p className="text-sm text-muted">{t("tunnel.sharing", { host: tunnel.host })}</p>
        <p className="break-all font-mono text-base font-medium select-all" dir="ltr">
          {tunnel.publicUrl}
        </p>
        <div className="flex flex-wrap gap-2">
          <Button primary onClick={() => void copy()} icon={copied ? <Check /> : <Copy />}>
            {copied ? t("tunnel.copied") : t("tunnel.copy")}
          </Button>
          <Button onClick={() => action.mutate(() => openUrl(tunnel.publicUrl))} icon={<ExternalLink />}>
            {t("tunnel.openPublic")}
          </Button>
          <Button onClick={() => action.mutate(() => tunnelStop(tunnel.name))} disabled={action.isPending} icon={<Square />}>
            {t("tunnel.stop")}
          </Button>
        </div>
        <p className="text-xs text-muted">{t("tunnel.phoneHint")}</p>
        <ErrorBox error={action.error} />
      </div>
    </div>
  );
}
