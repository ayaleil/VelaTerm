//! Desktop-only command wrappers for the optional Cloudflare Access listener on 127.0.0.1.

import { invoke } from "./transport";

/** Listener status and saved settings matching Rust `CloudflareAccessStatus` in camelCase. */
export interface CloudflareAccessStatus {
  running: boolean;
  port: number | null;
  /** Whether the listener auto-starts on the next launch. */
  enabled: boolean;
  teamDomain: string;
  aud: string;
  savedPort: number;
  allowedEmails: string[];
  /** Whether a stored access password exists, making the password field optional. */
  hasPassword: boolean;
  autostartError: string | null;
}

export interface CloudflareAccessConfig {
  teamDomain: string;
  aud: string;
  port: number;
  allowedEmails: string[];
  /** Empty reuses the stored access password. */
  password: string;
}

export function cloudflareAccessStatus(): Promise<CloudflareAccessStatus> {
  return invoke<CloudflareAccessStatus>("cloudflare_access_status");
}

export function cloudflareAccessStart(
  config: CloudflareAccessConfig,
): Promise<CloudflareAccessStatus> {
  return invoke<CloudflareAccessStatus>("cloudflare_access_start", {
    teamDomain: config.teamDomain,
    aud: config.aud,
    port: config.port,
    allowedEmails: config.allowedEmails,
    password: config.password || null,
  });
}

export function cloudflareAccessStop(): Promise<void> {
  return invoke<void>("cloudflare_access_stop");
}
