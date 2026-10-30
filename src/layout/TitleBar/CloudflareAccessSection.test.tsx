//! Coverage for the Cloudflare Access section: saved settings prefill the collapsed section, enabling sends
//! the normalized configuration, the active state shows the local URL with the tunnel hint, and both client
//! validation and backend start errors are surfaced, and the help popover links to the right Cloudflare pages.

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { CloudflareAccessStatus } from "../../ipc/cloudflareAccess";

const { statusMock, startMock, stopMock, openExternalMock } = vi.hoisted(() => ({
  statusMock: vi.fn(),
  startMock: vi.fn(),
  stopMock: vi.fn(),
  openExternalMock: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string, ...args: unknown[]) => (args.length ? `${key}:${args.join(",")}` : key),
  getLocale: () => "en",
}));
vi.mock("../../platform", () => ({
  platform: { opener: { openExternal: openExternalMock } },
}));
vi.mock("../../ipc/cloudflareAccess", () => ({
  cloudflareAccessStatus: statusMock,
  cloudflareAccessStart: startMock,
  cloudflareAccessStop: stopMock,
}));

import { CloudflareAccessSection, parseEmails } from "./CloudflareAccessSection";
import {
  AUD_DOCS_URL,
  OTP_DOCS_URL,
  TEAM_DOMAIN_DOCS_URL,
  TUNNEL_DOCS_URL,
  TUNNELS_URL,
  ZERO_TRUST_URL,
} from "./CloudflareAccessHelp";

function stopped(over: Partial<CloudflareAccessStatus> = {}): CloudflareAccessStatus {
  return {
    running: false,
    port: null,
    enabled: false,
    teamDomain: "",
    aud: "",
    savedPort: 8800,
    allowedEmails: [],
    hasPassword: true,
    autostartError: null,
    ...over,
  };
}

async function expand() {
  render(<CloudflareAccessSection />);
  await waitFor(() => expect(statusMock).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("button", { name: /remote.cloudflareTitle/ }));
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("CloudflareAccessSection", () => {
  it("starts collapsed and prefills saved settings when expanded", async () => {
    statusMock.mockResolvedValue(
      stopped({ teamDomain: "myteam.cloudflareaccess.com", aud: "tag", savedPort: 9000, allowedEmails: ["a@x.com", "b@x.com"] }),
    );
    render(<CloudflareAccessSection />);
    expect(screen.queryByText("remote.cloudflareDesc")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /remote.cloudflareTitle/ }));
    await waitFor(() =>
      expect(screen.getByPlaceholderText("myteam.cloudflareaccess.com")).toHaveProperty(
        "value",
        "myteam.cloudflareaccess.com",
      ),
    );
    expect(screen.getByPlaceholderText("8800")).toHaveProperty("value", "9000");
    expect(screen.getByPlaceholderText("remote.cloudflareEmailsPlaceholder")).toHaveProperty(
      "value",
      "a@x.com, b@x.com",
    );
    expect(screen.getByLabelText("remote.cloudflareAud")).toHaveProperty("type", "password");
  });

  it("enables the listener with the entered configuration and shows the local URL", async () => {
    statusMock.mockResolvedValue(stopped());
    startMock.mockResolvedValue(
      stopped({ running: true, enabled: true, port: 8800, teamDomain: "myteam.cloudflareaccess.com", aud: "tag" }),
    );
    await expand();
    fireEvent.change(screen.getByPlaceholderText("myteam.cloudflareaccess.com"), {
      target: { value: " https://myteam.cloudflareaccess.com " },
    });
    fireEvent.change(screen.getByLabelText("remote.cloudflareAud"), { target: { value: "tag" } });
    fireEvent.change(screen.getByPlaceholderText("remote.cloudflareEmailsPlaceholder"), {
      target: { value: "a@x.com, , b@x.com" },
    });
    fireEvent.click(screen.getByRole("checkbox"));

    await waitFor(() => expect(startMock).toHaveBeenCalledTimes(1));
    expect(startMock).toHaveBeenCalledWith({
      teamDomain: "https://myteam.cloudflareaccess.com",
      aud: "tag",
      port: 8800,
      allowedEmails: ["a@x.com", "b@x.com"],
      password: "",
    });
    await waitFor(() => expect(screen.getByText("http://127.0.0.1:8800")).toBeTruthy());
    expect(screen.getByText("remote.cloudflareHint")).toBeTruthy();
    expect(screen.getByTestId("cloudflare-active-dot")).toBeTruthy();
    expect(screen.getByRole("checkbox")).toHaveProperty("checked", true);
  });

  it("stops the listener when the toggle is turned off", async () => {
    statusMock
      .mockResolvedValueOnce(stopped({ running: true, enabled: true, port: 8800 }))
      .mockResolvedValueOnce(stopped());
    stopMock.mockResolvedValue(undefined);
    await expand();
    await waitFor(() => expect(screen.getByRole("checkbox")).toHaveProperty("checked", true));
    fireEvent.click(screen.getByRole("checkbox"));
    await waitFor(() => expect(stopMock).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByRole("checkbox")).toHaveProperty("checked", false));
    expect(screen.queryByText("remote.cloudflareHint")).toBeNull();
  });

  it("validates required fields before calling the backend", async () => {
    statusMock.mockResolvedValue(stopped({ hasPassword: false }));
    await expand();
    fireEvent.click(screen.getByRole("checkbox"));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "remote.cloudflareTeamDomainRequired");

    fireEvent.change(screen.getByPlaceholderText("myteam.cloudflareaccess.com"), {
      target: { value: "myteam.cloudflareaccess.com" },
    });
    fireEvent.click(screen.getByRole("checkbox"));
    expect(await screen.findByText("remote.cloudflareAudRequired")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("remote.cloudflareAud"), { target: { value: "tag" } });
    fireEvent.click(screen.getByRole("checkbox"));
    expect(await screen.findByText("remote.needPassword")).toBeTruthy();
    expect(startMock).not.toHaveBeenCalled();
  });

  it("surfaces backend start errors and auto-start failures", async () => {
    statusMock.mockResolvedValue(
      stopped({ teamDomain: "evil.com", aud: "tag", autostartError: "Port 8800 is already in use" }),
    );
    startMock.mockRejectedValue("Team domain must look like <team>.cloudflareaccess.com");
    await expand();
    expect(await screen.findByText(/Port 8800 is already in use/)).toBeTruthy();
    fireEvent.click(screen.getByRole("checkbox"));
    expect(
      await screen.findByText("Team domain must look like <team>.cloudflareaccess.com"),
    ).toBeTruthy();
    expect(screen.getByRole("checkbox")).toHaveProperty("checked", false);
  });

  it("opens a help popover with each Cloudflare step and external links", async () => {
    statusMock.mockResolvedValue(stopped({ savedPort: 9100 }));
    openExternalMock.mockResolvedValue(undefined);
    render(<CloudflareAccessSection />);
    await waitFor(() => expect(statusMock).toHaveBeenCalled());
    expect(screen.queryByRole("dialog")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "remote.cloudflareHelp" }));
    const dialog = await screen.findByRole("dialog", { name: "remote.cloudflareHelp" });
    for (const step of [
      "remote.cloudflareHelpTeamDomain",
      "remote.cloudflareHelpAud",
      "remote.cloudflareHelpEmail",
    ]) {
      expect(within(dialog).getByText(step)).toBeTruthy();
    }
    await waitFor(() => expect(within(dialog).getByText("remote.cloudflareHelpTunnel:9100")).toBeTruthy());

    const hrefs = within(dialog)
      .getAllByRole("link")
      .map((a) => a.getAttribute("href"));
    expect(new Set(hrefs)).toEqual(
      new Set([ZERO_TRUST_URL, TUNNELS_URL, TEAM_DOMAIN_DOCS_URL, AUD_DOCS_URL, TUNNEL_DOCS_URL, OTP_DOCS_URL]),
    );
    expect(ZERO_TRUST_URL).toBe("https://one.dash.cloudflare.com/");
    expect(TUNNELS_URL).toBe("https://dash.cloudflare.com/?to=/:account/tunnels");
    for (const a of within(dialog).getAllByRole("link")) {
      expect(a.getAttribute("target")).toBe("_blank");
      expect(a.getAttribute("rel")).toBe("noopener noreferrer");
    }

    fireEvent.click(within(dialog).getAllByRole("link", { name: /remote.cloudflareHelpDocs/ })[1]);
    expect(openExternalMock).toHaveBeenCalledWith(AUD_DOCS_URL);

    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("parses comma-separated emails", () => {
    expect(parseEmails(" a@x.com,,b@x.com , ")).toEqual(["a@x.com", "b@x.com"]);
  });
});
