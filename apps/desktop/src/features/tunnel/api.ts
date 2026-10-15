// IPC wrappers for the `tunnel` feature (see src-tauri/src/commands/tunnel.rs).
import { call } from "@/lib/ipc";

/** Mirrors `TunnelView`. */
export interface TunnelView {
  /** The site's folder name. */
  name: string;
  /** `https://abc123.ngrok-free.app`. */
  publicUrl: string;
  /** The site's local domain, e.g. `blog.test`. */
  host: string;
  port: number;
  /** Unix seconds. */
  startedAt: number;
  /** QR code of `publicUrl` (an SVG document). */
  qrSvg: string;
}

/** Mirrors `TunnelOverview`. */
export interface TunnelOverview {
  installed: string | null;
  available: string | null;
  hasToken: boolean;
  tokenError: string | null;
  /** Saved reserved domain, or "". */
  domain: string;
  tunnels: TunnelView[];
  /** Why the ngrok agent stopped by itself. */
  stopped: string | null;
  logFile: string;
}

export const TUNNEL_KEY = ["tunnel"] as const;

export const tunnelOverview = () => call<TunnelOverview>("tunnel_overview");
export const tunnelSetToken = (token: string) => call<void>("tunnel_set_token", { token });
export const tunnelClearToken = () => call<void>("tunnel_clear_token");
/** Returns the domain as saved (normalized). */
export const tunnelSetDomain = (domain: string) => call<string>("tunnel_set_domain", { domain });
/** `domain`: a reserved domain to use; `null` = a random address. */
export const tunnelStart = (name: string, domain: string | null) => call<TunnelView>("tunnel_start", { name, domain });
export const tunnelStop = (name: string) => call<void>("tunnel_stop", { name });
export const tunnelStopAll = () => call<void>("tunnel_stop_all");
export const tunnelOpenLog = () => call<void>("tunnel_open_log");

/** Where ngrok shows a user's authtoken. */
export const TOKEN_URL = "https://dashboard.ngrok.com/get-started/your-authtoken";
export const SIGNUP_URL = "https://dashboard.ngrok.com/signup";
export const DOMAINS_URL = "https://dashboard.ngrok.com/domains";

/** The QR code as an <img> source. */
export function qrSource(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** Error codes that mean "set up sharing first". */
export const SETUP_ERRORS = ["tunnel_not_installed", "tunnel_no_token", "tunnel_auth", "tunnel_invalid_token"];
