import { useTranslation } from "react-i18next";
import { Download } from "lucide-react";
import { describeError } from "@/lib/errors";
import { missingOf } from "@/lib/ipc";

/** A failed command: translated headline plus the backend's English detail. */
export function ErrorBox({ error }: { error: unknown }) {
  const { t } = useTranslation();
  if (!error) return null;
  const { title, detail } = describeError(t, error);
  const missing = missingOf(error);
  if (missing) {
    // Not a failure: something to install. Offer the way there.
    return (
      <div role="alert" className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-accent/40 bg-accent-soft px-3 py-2 text-sm">
        <span>
          {title} <span className="text-fg-soft">{t("errors.not_installed_hint")}</span>
        </span>
        {/* A plain link: the app uses a hash router, and this box also renders outside it. */}
        <a
          href={`#/packages?package=${encodeURIComponent(missing.package)}`}
          className="flex shrink-0 items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 font-medium text-on-accent outline-none hover:bg-accent-strong focus-visible:ring-3 focus-visible:ring-accent/50 [&_svg]:size-4"
        >
          <Download aria-hidden />
          {t("errors.install", { name: missing.name })}
        </a>
      </div>
    );
  }
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
