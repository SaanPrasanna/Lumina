import { TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ConfigWarning } from "@/types";

/** Lists config problems the backend found (and usually repaired). */
export function ConfigWarnings({ warnings }: { warnings: ConfigWarning[] }) {
  const { t } = useTranslation();
  if (warnings.length === 0) return null;

  return (
    <ul role="status" className="space-y-1.5 rounded-lg border border-warn/40 bg-warn-soft p-3 text-sm">
      {warnings.map((w, i) => (
        <li key={`${w.code}-${w.field}-${i}`} className="flex gap-2 text-warn">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
          <span>{t(`configWarnings.${w.code}`, { ...w.values, defaultValue: w.message })}</span>
        </li>
      ))}
    </ul>
  );
}
