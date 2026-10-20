import { afterEach, describe, expect, it } from "vitest";
import { remoteSshTarget } from "../ipc/transport";
import { vscodeRemoteUri } from "./vscodeRemote";

const hex = (value: string) =>
  Array.from(new TextEncoder().encode(value), (b) => b.toString(16).padStart(2, "0")).join("");

describe("vscodeRemoteUri", () => {
  it("uses the SSH target as the Remote-SSH authority", () => {
    expect(vscodeRemoteUri("edu@server.ts.net", "/home/edu/projects/app")).toBe(
      "vscode://vscode-remote/ssh-remote+edu@server.ts.net/home/edu/projects/app",
    );
  });

  it("encodes a target with a port as a hex JSON authority", () => {
    const authority = hex(JSON.stringify({ hostName: "10.0.0.2", user: "edu", port: 2222 }));
    expect(vscodeRemoteUri("edu@10.0.0.2:2222", "/srv/app")).toBe(
      `vscode://vscode-remote/ssh-remote+${authority}/srv/app`,
    );
  });

  it("omits the user from the hex authority when the target has none", () => {
    const authority = hex(JSON.stringify({ hostName: "server", port: 2222 }));
    expect(vscodeRemoteUri("server:2222", "/srv/app")).toBe(
      `vscode://vscode-remote/ssh-remote+${authority}/srv/app`,
    );
  });

  it("percent-encodes path segments that would break the URI", () => {
    expect(vscodeRemoteUri("edu@server", "/home/edu/my app/#1")).toBe(
      "vscode://vscode-remote/ssh-remote+edu@server/home/edu/my%20app/%231",
    );
  });

  it("turns a Windows server path into a slash-rooted path", () => {
    expect(vscodeRemoteUri("edu@winbox", "C:\\Users\\edu\\app")).toBe(
      "vscode://vscode-remote/ssh-remote+edu@winbox/C:/Users/edu/app",
    );
  });
});

describe("remoteSshTarget", () => {
  const w = window as unknown as { __VLX_REMOTE__?: unknown };
  afterEach(() => {
    delete w.__VLX_REMOTE__;
  });

  it("returns the address of an SSH connection window", () => {
    w.__VLX_REMOTE__ = { address: "edu@server", session: "s1", transport: "ssh" };
    expect(remoteSshTarget()).toBe("edu@server");
  });

  it("is null for WSL windows, paired URL windows, and the local app", () => {
    w.__VLX_REMOTE__ = { address: "Ubuntu", session: "s1", transport: "wsl" };
    expect(remoteSshTarget()).toBeNull();
    w.__VLX_REMOTE__ = { address: "https://10.0.0.2:18732" };
    expect(remoteSshTarget()).toBeNull();
    delete w.__VLX_REMOTE__;
    expect(remoteSshTarget()).toBeNull();
  });
});
