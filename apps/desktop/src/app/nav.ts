import {
  Globe,
  Layers,
  LayoutDashboard,
  PackagePlus,
  Rocket,
  ScrollText,
  Server,
  Settings,
  Share2,
  Terminal,
  type LucideIcon,
} from "lucide-react";

export interface NavItem {
  path: string;
  /** i18n key under `nav.*`. */
  labelKey: string;
  icon: LucideIcon;
}

/** Sidebar order. Each path maps to a route in router.tsx. */
export const NAV_ITEMS: NavItem[] = [
  { path: "/", labelKey: "nav.dashboard", icon: LayoutDashboard },
  { path: "/services", labelKey: "nav.services", icon: Server },
  { path: "/sites", labelKey: "nav.sites", icon: Globe },
  { path: "/packages", labelKey: "nav.packages", icon: PackagePlus },
  { path: "/profiles", labelKey: "nav.profiles", icon: Layers },
  { path: "/quickapp", labelKey: "nav.quickapp", icon: Rocket },
  { path: "/tunnel", labelKey: "nav.tunnel", icon: Share2 },
  { path: "/env-path", labelKey: "nav.envPath", icon: Terminal },
  { path: "/logs", labelKey: "nav.logs", icon: ScrollText },
  { path: "/settings", labelKey: "nav.settings", icon: Settings },
];
