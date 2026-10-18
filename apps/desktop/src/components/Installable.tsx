import { Download } from "lucide-react";
import { useTranslation } from "react-i18next";

/** Shown in place of a service's status while nothing is installed to run. */
export function NotInstalledBadge() {
  const { t } = useTranslation();
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-line px-2 py-0.5 text-xs font-medium text-muted">
      <Download className="size-3" aria-hidden />
      {t("status.notInstalled")}
    </span>
  );
}

/**
 * Opens Quick Add on `pkg`, in place of a Start button that could not work.
 * A plain link: the app uses a hash router, and this also renders outside it.
 */
export function InstallButton({ pkg, name, primary = true }: { pkg: string; name: string; primary?: boolean }) {
  const { t } = useTranslation();
  return (
    <a
      href={`#/packages?package=${encodeURIComponent(pkg)}`}
      aria-label={t("errors.install", { name })}
      className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium outline-none transition-colors focus-visible:ring-3 focus-visible:ring-accent/50 [&_svg]:size-4 ${
        primary ? "bg-accent text-on-accent hover:bg-accent-strong" : "border border-line hover:bg-hover"
      }`}
    >
      <Download aria-hidden />
      {t("packages.install")}
    </a>
  );
}
