import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { type Request, siteRequests } from "@/features/sites/api";

const LIMIT = 50;

function statusTone(status: number): string {
  if (status >= 500) return "text-danger";
  if (status >= 400) return "text-warn";
  return "text-muted";
}

/** The newest requests to a site, refreshed every few seconds while shown. */
export function Requests({ name }: { name: string }) {
  const { t, i18n } = useTranslation();
  const requests = useQuery({
    queryKey: ["sites", "requests", name],
    queryFn: () => siteRequests(name, LIMIT),
    refetchInterval: 3000,
  });
  const rows: Request[] = requests.data ?? [];
  const time = (iso: string) => {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso : d.toLocaleTimeString(i18n.language);
  };

  return (
    <div className="mt-3 rounded-md border border-line">
      {rows.length === 0 ? (
        <p className="px-3 py-2.5 text-sm text-muted">{t("sites.requests.empty")}</p>
      ) : (
        <div className="max-h-72 overflow-auto">
          <table className="w-full text-start font-mono text-xs" dir="ltr">
            <thead className="sticky top-0 bg-surface text-muted">
              <tr>
                <th className="px-3 py-1.5 font-medium">{t("sites.requests.time")}</th>
                <th className="px-3 py-1.5 font-medium">{t("sites.requests.request")}</th>
                <th className="px-3 py-1.5 text-end font-medium">{t("sites.requests.status")}</th>
                <th className="px-3 py-1.5 text-end font-medium">{t("sites.requests.ms")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {rows.map((r, i) => (
                <tr key={`${r.time}-${i}`}>
                  <td className="whitespace-nowrap px-3 py-1 text-muted">{time(r.time)}</td>
                  <td className="max-w-0 truncate px-3 py-1" title={`${r.method} ${r.host}${r.uri}`}>
                    {t("sites.requests.line", { method: r.method, uri: r.uri })}
                  </td>
                  <td className={`px-3 py-1 text-end ${statusTone(r.status)}`}>{r.status}</td>
                  <td className="px-3 py-1 text-end text-muted">{Math.round(r.ms ?? 0)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
