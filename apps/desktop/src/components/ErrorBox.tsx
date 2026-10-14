import { useTranslation } from "react-i18next";
import { describeError } from "@/lib/errors";

/** A failed command: translated headline plus the backend's English detail. */
export function ErrorBox({ error }: { error: unknown }) {
  const { t } = useTranslation();
  if (!error) return null;
  const { title, detail } = describeError(t, error);
  return (
    <div role="alert" className="rounded-md bg-danger-soft px-3 py-2 text-sm text-danger">
      {title}
      {detail && (
        <pre className="mt-1 whitespace-pre-wrap break-words font-mono text-xs opacity-80" dir="ltr">
          {detail}
        </pre>
      )}
    </div>
  );
}
