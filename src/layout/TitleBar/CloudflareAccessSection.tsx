//! Collapsible Cloudflare Access section of the remote-access panel. It configures the optional loopback
//! listener that sits behind the user's own cloudflared tunnel; VelaTerm only validates Access tokens and
//! never creates tunnels, applications, or DNS records.

import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { PasswordField } from "../../components/PasswordField";
import { CloudflareAccessHelp } from "./CloudflareAccessHelp";
import {
  cloudflareAccessStart,
  cloudflareAccessStatus,
  cloudflareAccessStop,
  type CloudflareAccessStatus,
} from "../../ipc/cloudflareAccess";

const DEFAULT_PORT = "8800";

const inputStyle: React.CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "8px 10px",
  border: "1px solid var(--border)",
  borderRadius: "var(--r-sm, 6px)",
  background: "var(--bg-0)",
  color: "var(--text)",
  fontSize: 13,
  outline: "none",
};

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 11,
  color: "var(--text-dim)",
  marginBottom: 4,
};

export function parseEmails(raw: string): string[] {
  return raw
    .split(",")
    .map((e) => e.trim())
    .filter((e) => e.length > 0);
}

export function CloudflareAccessSection() {
  const t = useT();
  const [status, setStatus] = useState<CloudflareAccessStatus | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [teamDomain, setTeamDomain] = useState("");
  const [aud, setAud] = useState("");
  const [port, setPort] = useState(DEFAULT_PORT);
  const [emails, setEmails] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const applyStatus = (s: CloudflareAccessStatus) => {
    setStatus(s);
    setTeamDomain(s.teamDomain);
    setAud(s.aud);
    setPort(String(s.port ?? s.savedPort));
    setEmails(s.allowedEmails.join(", "));
  };

  useEffect(() => {
    cloudflareAccessStatus()
      .then(applyStatus)
      .catch(() => setStatus(null));
  }, []);

  const running = status?.running ?? false;

  const start = async () => {
    if (!teamDomain.trim()) {
      setError(t("remote.cloudflareTeamDomainRequired"));
      return;
    }
    if (!aud.trim()) {
      setError(t("remote.cloudflareAudRequired"));
      return;
    }
    const portNum = Number(port.trim());
    if (!Number.isInteger(portNum) || portNum < 1 || portNum > 65535) {
      setError(t("remote.portInvalid"));
      return;
    }
    if (!password.trim() && !status?.hasPassword) {
      setError(t("remote.needPassword"));
      return;
    }
    setBusy(true);
    setError("");
    try {
      applyStatus(
        await cloudflareAccessStart({
          teamDomain: teamDomain.trim(),
          aud: aud.trim(),
          port: portNum,
          allowedEmails: parseEmails(emails),
          password: password.trim(),
        }),
      );
      setPassword("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    setError("");
    try {
      await cloudflareAccessStop();
      applyStatus(await cloudflareAccessStatus());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const localUrl = `http://127.0.0.1:${status?.port ?? port}`;
  const fieldsDisabled = running || busy;

  return (
    <div
      style={{
        marginTop: 14,
        paddingTop: 12,
        borderTop: "1px solid var(--border)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <button
          type="button"
          aria-expanded={expanded}
          onClick={() => setExpanded((v) => !v)}
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 8,
            padding: 0,
            border: "none",
            background: "transparent",
            color: "var(--text-dim)",
            fontSize: 10.5,
            letterSpacing: "1px",
            textTransform: "uppercase",
            fontWeight: 600,
            cursor: "pointer",
          }}
        >
          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
            {running && (
              <span
                data-testid="cloudflare-active-dot"
                style={{
                  width: 6,
                  height: 6,
                  borderRadius: "50%",
                  background: "var(--accent)",
                }}
              />
            )}
            {t("remote.cloudflareTitle")}
          </span>
          <span aria-hidden>{expanded ? "▾" : "▸"}</span>
        </button>
        <CloudflareAccessHelp port={String(status?.port ?? (port || DEFAULT_PORT))} />
      </div>

      {expanded && (
        <div style={{ marginTop: 10 }}>
          <div
            style={{
              fontSize: 11,
              color: "var(--text-dim)",
              lineHeight: 1.5,
              marginBottom: 10,
            }}
          >
            {t("remote.cloudflareDesc")}
          </div>

          <label
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              marginBottom: 10,
              fontSize: 12.5,
              color: "var(--text)",
              cursor: busy ? "default" : "pointer",
            }}
          >
            <input
              type="checkbox"
              checked={running}
              disabled={busy}
              onChange={(e) => void (e.target.checked ? start() : stop())}
              style={{ cursor: busy ? "default" : "pointer" }}
            />
            {t("remote.cloudflareEnable")}
          </label>

          <label style={labelStyle}>
            {t("remote.cloudflareTeamDomain")}
            <input
              type="text"
              value={teamDomain}
              placeholder="myteam.cloudflareaccess.com"
              disabled={fieldsDisabled}
              onChange={(e) => setTeamDomain(e.target.value)}
              style={{ ...inputStyle, marginTop: 4 }}
            />
          </label>
          <div style={{ height: 8 }} />
          <span style={labelStyle}>{t("remote.cloudflareAud")}</span>
          {/* Masked on purpose: the panel is often visible while screen sharing, and the AUD tag identifies the Access app. */}
          <PasswordField
            value={aud}
            ariaLabel={t("remote.cloudflareAud")}
            disabled={fieldsDisabled}
            onChange={setAud}
            wrapStyle={{ marginBottom: 8 }}
          />
          <label style={labelStyle}>
            {t("remote.cloudflarePort")}
            <input
              type="text"
              inputMode="numeric"
              value={port}
              placeholder={DEFAULT_PORT}
              disabled={fieldsDisabled}
              onChange={(e) => setPort(e.target.value.replace(/[^0-9]/g, ""))}
              style={{ ...inputStyle, marginTop: 4 }}
            />
          </label>
          <div style={{ height: 8 }} />
          <label style={labelStyle}>
            {t("remote.cloudflareEmails")}
            <input
              type="text"
              value={emails}
              placeholder={t("remote.cloudflareEmailsPlaceholder")}
              disabled={fieldsDisabled}
              onChange={(e) => setEmails(e.target.value)}
              style={{ ...inputStyle, marginTop: 4 }}
            />
          </label>
          {!running && (
            <>
              <div style={{ height: 8 }} />
              <PasswordField
                value={password}
                ariaLabel={t("remote.cloudflarePassword")}
                placeholder={
                  status?.hasPassword
                    ? t("remote.cloudflarePasswordSaved")
                    : t("remote.cloudflarePassword")
                }
                disabled={busy}
                onChange={setPassword}
              />
            </>
          )}

          {running && (
            <div
              style={{
                marginTop: 10,
                fontSize: 11.5,
                color: "var(--text)",
                lineHeight: 1.5,
              }}
            >
              <div>
                {t("remote.cloudflareActive")} <code>{localUrl}</code>
              </div>
              <div style={{ color: "var(--text-dim)" }}>
                {t("remote.cloudflareHint")}
              </div>
            </div>
          )}
        </div>
      )}

      {status?.autostartError && (
        <div
          style={{
            marginTop: 10,
            fontSize: 11,
            color: "var(--danger, #ff6b6b)",
            lineHeight: 1.4,
          }}
        >
          {t("remote.autostartFailed")} {status.autostartError}
        </div>
      )}
      {error && (
        <div
          role="alert"
          style={{
            marginTop: 10,
            fontSize: 11,
            color: "var(--danger, #ff6b6b)",
            lineHeight: 1.4,
          }}
        >
          {error}
        </div>
      )}
    </div>
  );
}
