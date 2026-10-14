import { useQuery } from "@tanstack/react-query";
import { FolderOpen, Globe, Play, Square } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ConfigWarnings } from "@/components/ConfigWarnings";
import { ErrorBox } from "@/components/ErrorBox";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui";
import {
  openLocalhost,
  openWww,
  SERVICES_KEY,
  servicesStartAll,
  servicesStatus,
  servicesStopAll,
} from "@/features/services/api";
import { PhpCard } from "@/features/services/PhpCard";
import { PortConflictDialog } from "@/features/services/PortConflictDialog";
import { useServiceAction } from "@/features/services/useServiceAction";
import { WebCard } from "@/features/services/WebCard";
import { SETTINGS_KEY, settingsGet } from "@/features/settings/api";
import { isCommandError, type PortConflict, portConflictOf } from "@/lib/ipc";

export function ServicesPage() {
  const { t } = useTranslation();
  const settings = useQuery({ queryKey: SETTINGS_KEY, queryFn: settingsGet });
  const stack = useQuery({
    queryKey: SERVICES_KEY,
    queryFn: servicesStatus,
    // Process counts change without events (workers recycling); poll while up.
    refetchInterval: (q) => (q.state.data?.php.runtime || q.state.data?.web.runtime ? 5000 : false),
  });
  const action = useServiceAction();
  const [conflict, setConflict] = useState<PortConflict | null>(null);

  const config = settings.data?.config;
  const anyRunning = !!(stack.data?.php.runtime || stack.data?.web.runtime);

  const startAll = () =>
    action.mutate(servicesStartAll, {
      onError: (err) => setConflict(portConflictOf(err, config?.web.http_port)),
    });

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
            <WebCard status={stack.data.web} config={config?.web} onPortConflict={setConflict} />
          </>
        )}
      </div>
      {conflict && config && (
        <PortConflictDialog conflict={conflict} web={config.web} onClose={() => setConflict(null)} />
      )}
    </>
  );
}
