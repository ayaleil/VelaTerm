//! Help popover for the Cloudflare Access section: where each value lives in the user's own Cloudflare
//! account, with account-agnostic dashboard links and the matching Cloudflare docs pages.

import { useEffect, useRef, useState } from "react";
import { useT } from "../../i18n";
import Icons from "../../components/Icons";
import { SELECT_PANEL, useMenuPosition } from "../../components/Select";
import { platform } from "../../platform";

export const ZERO_TRUST_URL = "https://one.dash.cloudflare.com/";
export const TUNNELS_URL = "https://dash.cloudflare.com/?to=/:account/tunnels";
export const TEAM_DOMAIN_DOCS_URL =
  "https://developers.cloudflare.com/cloudflare-one/faq/getting-started-faq/#what-is-a-team-domainteam-name";
export const AUD_DOCS_URL =
  "https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/authorization-cookie/validating-json/#get-your-aud-tag";
export const TUNNEL_DOCS_URL =
  "https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/get-started/create-remote-tunnel/#2a-publish-an-application";
export const OTP_DOCS_URL =
  "https://developers.cloudflare.com/cloudflare-one/integrations/identity-providers/one-time-pin/";

const POPOVER_WIDTH = 340;

function ExternalLink({ href, label }: { href: string; label: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(event) => {
        event.preventDefault();
        void platform.opener.openExternal(event.currentTarget.href).catch(() => {});
      }}
      style={{ display: "inline-flex", alignItems: "center", gap: 3, color: "var(--accent)", textDecoration: "none" }}
    >
      {label}
      <Icons.external aria-hidden="true" size={11} />
    </a>
  );
}

export function CloudflareAccessHelp({ port }: { port: string }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const boxRef = useRef<HTMLSpanElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const position = useMenuPosition(boxRef, panelRef, open, true, POPOVER_WIDTH, "right");

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!boxRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const dashboard = t("remote.cloudflareHelpDashboard");
  const docs = t("remote.cloudflareHelpDocs");
  const items = [
    {
      title: t("remote.cloudflareTeamDomain"),
      body: t("remote.cloudflareHelpTeamDomain"),
      links: [
        { href: ZERO_TRUST_URL, label: dashboard },
        { href: TEAM_DOMAIN_DOCS_URL, label: docs },
      ],
    },
    {
      title: t("remote.cloudflareAud"),
      body: t("remote.cloudflareHelpAud"),
      links: [
        { href: ZERO_TRUST_URL, label: dashboard },
        { href: AUD_DOCS_URL, label: docs },
      ],
    },
    {
      title: t("remote.cloudflareHelpTunnelTitle"),
      body: t("remote.cloudflareHelpTunnel", port),
      links: [
        { href: TUNNELS_URL, label: dashboard },
        { href: TUNNEL_DOCS_URL, label: docs },
      ],
    },
    {
      title: t("remote.cloudflareHelpEmailTitle"),
      body: t("remote.cloudflareHelpEmail"),
      links: [
        { href: ZERO_TRUST_URL, label: dashboard },
        { href: OTP_DOCS_URL, label: docs },
      ],
    },
  ];

  return (
    <span ref={boxRef} style={{ display: "inline-flex" }}>
      <button
        type="button"
        aria-label={t("remote.cloudflareHelp")}
        title={t("remote.cloudflareHelp")}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        style={{
          display: "inline-flex",
          padding: 0,
          border: "none",
          background: "transparent",
          color: open ? "var(--text)" : "var(--text-dim)",
          cursor: "pointer",
        }}
      >
        <Icons.info aria-hidden="true" size={14} />
      </button>
      {open && (
        <div
          ref={panelRef}
          role="dialog"
          aria-label={t("remote.cloudflareHelp")}
          style={{
            ...SELECT_PANEL,
            ...position,
            padding: "10px 12px",
            fontSize: 11.5,
            lineHeight: 1.45,
            color: "var(--text)",
          }}
        >
          <div style={{ fontWeight: 600, marginBottom: 8 }}>{t("remote.cloudflareHelp")}</div>
          {items.map((item, i) => (
            <div key={item.title} style={{ marginTop: i === 0 ? 0 : 10 }}>
              <div style={{ fontWeight: 600 }}>{item.title}</div>
              <div style={{ color: "var(--text-dim)" }}>{item.body}</div>
              <div style={{ display: "flex", gap: 12, marginTop: 2 }}>
                {item.links.map((link) => (
                  <ExternalLink key={link.href} href={link.href} label={link.label} />
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </span>
  );
}
