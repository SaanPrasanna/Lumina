// IPC wrappers for the `tunnel` feature (see src-tauri/src/commands/tunnel.rs).
import { commands } from "@/lib/bindings";

export type { TunnelOverview, TunnelView } from "@/lib/bindings";

export const TUNNEL_KEY = ["tunnel"] as const;

export const tunnelOverview = () => commands.tunnelOverview();
export const tunnelSetToken = (token: string) => commands.tunnelSetToken(token);
export const tunnelClearToken = () => commands.tunnelClearToken();
/** Returns the domain as saved (normalized). */
export const tunnelSetDomain = (domain: string) => commands.tunnelSetDomain(domain);
/** `domain`: a reserved domain to use; `null` = a random address. */
export const tunnelStart = (name: string, domain: string | null) => commands.tunnelStart(name, domain);
export const tunnelStop = (name: string) => commands.tunnelStop(name);
export const tunnelStopAll = () => commands.tunnelStopAll();
export const tunnelOpenLog = () => commands.tunnelOpenLog();

/** Where ngrok shows a user's authtoken. */
export const TOKEN_URL = "https://dashboard.ngrok.com/get-started/your-authtoken";
export const SIGNUP_URL = "https://dashboard.ngrok.com/signup";
export const DOMAINS_URL = "https://dashboard.ngrok.com/domains";
/** Shape of a free static domain (an example, not translated). */
export const DOMAIN_EXAMPLE = "my-app.ngrok-free.app";

/** The QR code as an <img> source. */
export function qrSource(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** Error codes that mean "set up sharing first". */
export const SETUP_ERRORS = ["tunnel_not_installed", "tunnel_no_token", "tunnel_auth", "tunnel_invalid_token"];
