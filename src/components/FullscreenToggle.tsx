//! Fullscreen toggle for plain browser access (e.g. iPad Safari), so the browser chrome can be hidden.
//! Desktop windows (Tauri, Electron, remote-connection windows) have their own controls and never show it.

import Icons from "./Icons";
import { useT } from "../i18n";
import { env } from "../platform/env";
import { useFullscreen } from "../hooks/useFullscreen";

export function FullscreenToggle({ className, iconSize = 14 }: { className: string; iconSize?: number }) {
  const t = useT();
  const { supported, active, toggle } = useFullscreen();
  if (!env.isBrowser || env.isRemoteWindow || !supported) return null;
  const label = t(active ? "titlebar.exitFullscreen" : "titlebar.enterFullscreen");
  return (
    <button type="button" className={className} aria-pressed={active} title={label} aria-label={label} onClick={toggle}>
      {active ? <Icons.fullscreenExit size={iconSize} /> : <Icons.fullscreen size={iconSize} />}
    </button>
  );
}
