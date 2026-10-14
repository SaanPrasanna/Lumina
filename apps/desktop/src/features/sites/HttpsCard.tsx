import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { BadgeCheck, KeyRound, RotateCcw, ShieldAlert, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBox } from "@/components/ErrorBox";
import { Button } from "@/components/ui";
import { SETTINGS_KEY } from "@/features/settings/api";
import {
  type SslOverview,
  SSL_KEY,
  sslRegenerate,
  sslRemoveCa,
  sslSetEnabled,
  sslSetForceHttps,
  sslStatus,
  sslTrust,
  formatDate,
  type TrustOutcome,
} from "@/features/sites/ssl";

/** HTTPS on/off, "Force HTTPS", and the local certificate authority. */
export function HttpsCard() {
  const { t } = useTranslation();
  const ssl = useQuery({ queryKey: SSL_KEY, queryFn: sslStatus });
  const queryClient = useQueryClient();
  const [declined, setDeclined] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const action = useMutation({
    mutationFn: (fn: () => Promise<TrustOutcome | null | void>) => fn(),
    onMutate: () => setDeclined(false),
    onSuccess: (outcome) => setDeclined(outcome === "declined"),
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: ["sites"] }),
        queryClient.invalidateQueries({ queryKey: SETTINGS_KEY }),
      ]),
  });
  const busy = action.isPending;
  const data = ssl.data;

  return (
    <section className="rounded-xl border border-line p-5" aria-labelledby="https-title">
      <div className="flex items-center justify-between gap-3">
        <h2 id="https-title" className="text-lg font-semibold">
          {t("ssl.title")}
        </h2>
        {data && (
          <label className="flex items-center gap-2.5 text-sm font-medium">
            <input
              type="checkbox"
              role="switch"
              className="size-4 accent-accent"
              checked={data.enabled}
              disabled={busy}
              onChange={() => action.mutate(() => sslSetEnabled(!data.enabled))}
            />
            {data.enabled ? t("ssl.on") : t("ssl.off")}
          </label>
        )}
      </div>
      <p className="mt-1 text-sm text-muted">{t("ssl.description")}</p>

      <div className="mt-3 space-y-2">
        <ErrorBox error={ssl.error ?? action.error} />
        {declined && <p className="text-sm text-warn">{t("ssl.declined")}</p>}
      </div>

      {data?.enabled && (
        <div className="mt-4 space-y-4">
          <CaRow data={data} busy={busy} onTrust={() => action.mutate(sslTrust)} />
          {data.error && <ErrorBox error={{ code: "service", message: data.error }} />}

          <label className="flex items-center gap-2.5 text-sm">
            <input
              type="checkbox"
              className="size-4 accent-accent"
              checked={data.forceHttps}
              disabled={busy}
              onChange={() => action.mutate(() => sslSetForceHttps(!data.forceHttps))}
            />
            {t("ssl.forceHttps")}
          </label>
          <p className="text-xs text-muted">
            {t("ssl.port", { port: data.httpsPort })} {t("ssl.firefox")}
          </p>

          {data.ca && (
            <div className="flex flex-wrap gap-2 border-t border-line pt-4">
              <Button onClick={() => action.mutate(sslRegenerate)} disabled={busy} icon={<RotateCcw />}>
                {t("ssl.regenerate")}
              </Button>
              {confirmRemove ? (
                <>
                  <button
                    type="button"
                    onClick={() => {
                      setConfirmRemove(false);
                      action.mutate(sslRemoveCa);
                    }}
                    disabled={busy}
                    className="flex items-center gap-1.5 rounded-md border border-danger/50 px-3 py-1.5 text-sm text-danger outline-none hover:bg-danger-soft focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-50 [&_svg]:size-4"
                  >
                    <Trash2 />
                    {t("ssl.removeConfirm")}
                  </button>
                  <Button onClick={() => setConfirmRemove(false)} disabled={busy}>
                    {t("actions.cancel")}
                  </Button>
                </>
              ) : (
                <Button onClick={() => setConfirmRemove(true)} disabled={busy} icon={<Trash2 />}>
                  {t("ssl.remove")}
                </Button>
              )}
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function CaRow({ data, busy, onTrust }: { data: SslOverview; busy: boolean; onTrust: () => void }) {
  const { t, i18n } = useTranslation();
  const ca = data.ca;
  if (!ca) {
    return <p className="text-sm text-muted">{t("ssl.noCa")}</p>;
  }
  const trusted = ca.trusted && ca.coversTld;
  return (
    <div
      role="status"
      className={`flex flex-wrap items-center justify-between gap-3 rounded-md border px-4 py-3 text-sm ${
        trusted ? "border-line" : "border-warn/40 bg-warn-soft"
      }`}
    >
      <div className="flex min-w-0 items-start gap-2.5">
        {trusted ? <BadgeCheck className="mt-0.5 size-4 shrink-0 text-ok" /> : <ShieldAlert className="mt-0.5 size-4 shrink-0 text-warn" />}
        <div className="min-w-0">
          <p>
            {!ca.coversTld
              ? t("ssl.wrongTld", { tld: data.tld })
              : ca.trusted
                ? t("ssl.trusted")
                : t("ssl.untrusted")}
          </p>
          <p className="mt-0.5 truncate text-xs text-muted">
            <KeyRound className="me-1 inline size-3" />
            {t("ssl.caLine", { name: ca.subject, date: formatDate(ca.notAfter, i18n.language), interpolation: { escapeValue: false } })}
          </p>
        </div>
      </div>
      {!ca.trusted && ca.coversTld && (
        <Button primary onClick={onTrust} disabled={busy} icon={<BadgeCheck />}>
          {t("ssl.trust")}
        </Button>
      )}
    </div>
  );
}
