//! VS Code Remote-SSH deep links for SSH connection windows.
//!
//! A project in an SSH connection window lives on the server, but VS Code runs on this client. The link
//! `vscode://vscode-remote/ssh-remote+<authority><path>` asks the client's VS Code (with the Remote-SSH
//! extension) to connect to the same SSH target and open the folder there.

/** SSH target (`user@host[:port]`) of the current SSH connection window; null elsewhere (WSL, paired URLs, local). */
export function remoteSshTarget(): string | null {
  if (typeof window === "undefined") return null;
  const remote = (window as { __VLX_REMOTE__?: { address?: unknown; session?: unknown; transport?: unknown } })
    .__VLX_REMOTE__;
  if (!remote?.session || remote.transport !== "ssh" || typeof remote.address !== "string") return null;
  return remote.address || null;
}

/** Remote-SSH authority. `host:port` is not understood there, so a port uses the hex-encoded JSON form. */
function sshAuthority(target: string): string {
  // Same split as `split_ssh_target` in the backend: a numeric final segment is the port.
  const match = /^(.*):(\d+)$/.exec(target);
  if (!match) return target;
  const [, userHost, port] = match;
  const at = userHost.lastIndexOf("@");
  const spec = at >= 0
    ? { hostName: userHost.slice(at + 1), user: userHost.slice(0, at), port: Number(port) }
    : { hostName: userHost, port: Number(port) };
  return Array.from(new TextEncoder().encode(JSON.stringify(spec)), (b) => b.toString(16).padStart(2, "0")).join("");
}

/** Slash-rooted, percent-encoded server path; Windows drive paths become `/C:/...`. */
function uriPath(path: string): string {
  const segments = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return "/" + segments.map((s) => (/^[A-Za-z]:$/.test(s) ? s : encodeURIComponent(s))).join("/");
}

/** Deep link that opens `path` on the SSH `target` in the client's VS Code. */
export function vscodeRemoteUri(target: string, path: string): string {
  return `vscode://vscode-remote/ssh-remote+${sshAuthority(target)}${uriPath(path)}`;
}
