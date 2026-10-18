import { useQuery } from "@tanstack/react-query";
import { FolderOpen, Globe, Play, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ConfigWarnings } from "@/components/ConfigWarnings";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui";
import { openLocalhost, openWww, SERVICES_KEY, servicesStatus, servicesStopAll } from "@/features/services/api";
import { ExtraServices } from "@/features/services/ExtraServices";
import { SnapshotsCard } from "@/features/services/SnapshotsCard";
import { MailpitCard } from "@/features/services/MailpitCard";
import { PhpCard } from "@/features/services/PhpCard";
import { DatabaseCard } from "@/features/services/DatabaseCard";
import { PortConflictDialog } from "@/features/services/PortConflictDialog";
import { PostgresCard } from "@/features/services/PostgresCard";
import { RedisCard } from "@/features/services/RedisCard";
import { useStartAll } from "@/features/services/useStartAll";
import { WebCard } from "@/features/services/WebCard";
import { SETTINGS_KEY, settingsGet } from "@/features/settings/api";
import { isCommandError } from "@/lib/ipc";

export function ServicesPage() {
  const { t } = useTranslation();
  const settings = useQuery({ queryKey: SETTINGS_KEY, queryFn: settingsGet });
  const stack = useQuery({
    queryKey: SERVICES_KEY,
    queryFn: servicesStatus,
    // Process counts change without events (workers recycling); poll while up.
    refetchInterval: (q) => {
      const d = q.state.data;
      return d && (d.php.runtime || d.web.runtime || d.database.runtime || d.redis.runtime || d.postgresql.runtime || d.mailpit.runtime) ? 5000 : false;
    },
  });
  const config = settings.data?.config;
  const { action, startAll, conflict, setConflict } = useStartAll(config);
  const anyRunning = !!(
    stack.data?.php.runtime ||
    stack.data?.web.runtime ||
    stack.data?.database.runtime ||
    stack.data?.redis.runtime ||
    stack.data?.postgresql.runtime ||
    stack.data?.mailpit.runtime
  );

  return (
    <>
      <PageHeader
        title={t("nav.services")}
        actions={
          <div className="flex flex-wrap gap-2">
            {anyRunning ? (
              <Button onClick={() => action.mutate(servicesStopAll)} disabled={action.isPending} icon={<Square />}>
                {t("actions.stopAll")}
              </Button>
            ) : (
              <Button primary onClick={startAll} disabled={action.isPending} icon={<Play />}>
                {t("actions.startAll")}
              </Button>
            )}
            <Button onClick={() => action.mutate(openLocalhost)} disabled={!stack.data?.web.runtime} icon={<Globe />}>
              {t("actions.openLocalhost")}
            </Button>
            <Button onClick={() => action.mutate(openWww)} icon={<FolderOpen />}>
              {t("actions.openWww")}
            </Button>
          </div>
        }
      />
      <div className="space-y-4 px-8 pb-8">
        <ConfigWarnings warnings={settings.data?.warnings ?? []} />
        {!(isCommandError(action.error) && action.error.code === "port_in_use") && <ErrorBox error={action.error} />}
        {stack.data && (
          <>
            <PhpCard status={stack.data.php} config={config?.php} />
            {stack.data.phpPools.length > 0 && (
              <section className="rounded-xl border border-line px-5 py-3 text-sm" aria-label={t("php.pools.title")}>
                <h2 className="font-semibold">{t("php.pools.title")}</h2>
                <ul className="mt-1 space-y-0.5 text-muted">
                  {stack.data.phpPools.map((pool) => (
                    <li key={pool.logFile}>
                      {pool.runtime ? t("php.pools.running", { version: pool.runtime.version, port: pool.runtime.port }) : t(`status.${pool.status}`)}
                    </li>
                  ))}
                </ul>
              </section>
            )}
            <WebCard status={stack.data.web} config={config?.web} onPortConflict={setConflict} />
            <DatabaseCard status={stack.data.database} config={config?.database} onPortConflict={setConflict} />
            <PostgresCard status={stack.data.postgresql} config={config?.postgresql} onPortConflict={setConflict} />
            <RedisCard status={stack.data.redis} config={config?.redis} onPortConflict={setConflict} />
            <MailpitCard status={stack.data.mailpit} config={config?.mailpit} onPortConflict={setConflict} />
            <SnapshotsCard
              engines={[
                ...(stack.data.database.runtime ? [stack.data.database.engine] : []),
                ...(stack.data.postgresql.runtime ? ["postgresql"] : []),
              ]}
            />
            <ExtraServices config={config} />
          </>
        )}
      </div>
      {conflict && <PortConflictDialog request={conflict} onClose={() => setConflict(null)} />}
    </>
  );
}
