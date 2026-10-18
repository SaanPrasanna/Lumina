import { useQuery } from "@tanstack/react-query";
import {
  dbVersions,
  dbVersionsKey,
  MAILPIT_VERSIONS_KEY,
  mailpitVersions,
  PHP_VERSIONS_KEY,
  POSTGRESQL_VERSIONS_KEY,
  phpVersions,
  postgresqlVersions,
  REDIS_VERSIONS_KEY,
  redisVersions,
  webVersions,
  webVersionsKey,
} from "@/features/services/api";
import type { ServiceKind } from "@/lib/bindings";

/** The installed-versions query of a service: the same one its Services card uses. */
function versionsQuery(kind: ServiceKind) {
  switch (kind) {
    case "php-fcgi":
      return { queryKey: PHP_VERSIONS_KEY, queryFn: phpVersions as () => Promise<unknown[]> };
    case "nginx":
    case "apache":
      return { queryKey: webVersionsKey(kind), queryFn: () => webVersions(kind) as Promise<unknown[]> };
    case "mysql":
    case "mariadb":
      return { queryKey: dbVersionsKey(kind), queryFn: () => dbVersions(kind) as Promise<unknown[]> };
    case "postgresql":
      return { queryKey: POSTGRESQL_VERSIONS_KEY, queryFn: postgresqlVersions as () => Promise<unknown[]> };
    case "redis":
      return { queryKey: REDIS_VERSIONS_KEY, queryFn: redisVersions as () => Promise<unknown[]> };
    case "mailpit":
      return { queryKey: MAILPIT_VERSIONS_KEY, queryFn: mailpitVersions as () => Promise<unknown[]> };
  }
}

/** `false` once it is known that no version of `kind` is installed. */
export function useInstalled(kind: ServiceKind): boolean | undefined {
  const query = useQuery(versionsQuery(kind) as { queryKey: readonly unknown[]; queryFn: () => Promise<unknown[]> });
  return query.data === undefined ? undefined : query.data.length > 0;
}

/** The Quick Add package of a service. */
export const packageOf = (kind: ServiceKind) => (kind === "php-fcgi" ? "php" : kind);
