import { useState } from "react";
import { dbConflictRequest } from "@/features/services/DatabaseCard";
import { mailpitConflictRequest } from "@/features/services/MailpitCard";
import type { PortConflictRequest } from "@/features/services/PortConflictDialog";
import { postgresConflictRequest } from "@/features/services/PostgresCard";
import { redisConflictRequest } from "@/features/services/RedisCard";
import { servicesStartAll } from "@/features/services/api";
import { useServiceAction } from "@/features/services/useServiceAction";
import { webConflictRequest } from "@/features/services/WebCard";
import type { AppConfig } from "@/features/settings/api";
import { portConflictOf } from "@/lib/ipc";

/**
 * "Start All" for the Services page and the dashboard: a busy port opens the
 * "use another port" offer for whichever service hit it.
 */
export function useStartAll(config: AppConfig | undefined) {
  const action = useServiceAction();
  const [conflict, setConflict] = useState<PortConflictRequest | null>(null);

  const startAll = () =>
    action.mutate(servicesStartAll, {
      onError: (err) => {
        if (!config) return;
        const web = portConflictOf(err, config.web.http_port) ?? portConflictOf(err, config.web.https_port);
        const db = portConflictOf(err, config.database.port);
        const redis = portConflictOf(err, config.redis.port);
        const pg = portConflictOf(err, config.postgresql.port);
        if (web) setConflict(webConflictRequest(web, config.web));
        else if (db) setConflict(dbConflictRequest(db, config.database));
        else if (pg) setConflict(postgresConflictRequest(pg, config.postgresql));
        else if (redis) setConflict(redisConflictRequest(redis, config.redis));
        else {
          const mail = mailpitConflictRequest(err, config.mailpit);
          if (mail) setConflict(mail);
        }
      },
    });

  return { action, startAll, conflict, setConflict };
}
