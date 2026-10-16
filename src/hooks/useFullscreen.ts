//! Document-level Fullscreen API state, with the `webkit`-prefixed fallback iPad Safari still requires.

import { useCallback, useEffect, useState } from "react";

interface PrefixedDocument {
  webkitFullscreenEnabled?: boolean;
  webkitFullscreenElement?: Element | null;
  webkitExitFullscreen?: () => Promise<void> | void;
}

interface PrefixedElement {
  webkitRequestFullscreen?: () => Promise<void> | void;
}

const prefixedDoc = (): Document & PrefixedDocument => document;
const rootElement = (): HTMLElement & PrefixedElement => document.documentElement;

export function isFullscreenSupported(): boolean {
  if (typeof document === "undefined") return false;
  const doc = prefixedDoc();
  const root = rootElement();
  return (
    (!!doc.fullscreenEnabled && typeof root.requestFullscreen === "function") ||
    (!!doc.webkitFullscreenEnabled && typeof root.webkitRequestFullscreen === "function")
  );
}

function currentFullscreenElement(): Element | null {
  const doc = prefixedDoc();
  return doc.fullscreenElement ?? doc.webkitFullscreenElement ?? null;
}

function enter(): Promise<void> | void {
  const root = rootElement();
  if (typeof root.requestFullscreen === "function") return root.requestFullscreen();
  return root.webkitRequestFullscreen?.();
}

function exit(): Promise<void> | void {
  const doc = prefixedDoc();
  if (typeof doc.exitFullscreen === "function") return doc.exitFullscreen();
  return doc.webkitExitFullscreen?.();
}

/** Tracks whether the page is fullscreen (including exits via Esc or system gestures) and toggles it. */
export function useFullscreen(): { supported: boolean; active: boolean; toggle: () => void } {
  const [supported] = useState(isFullscreenSupported);
  const [active, setActive] = useState(() => supported && currentFullscreenElement() !== null);

  useEffect(() => {
    if (!supported) return;
    const sync = () => setActive(currentFullscreenElement() !== null);
    document.addEventListener("fullscreenchange", sync);
    document.addEventListener("webkitfullscreenchange", sync);
    return () => {
      document.removeEventListener("fullscreenchange", sync);
      document.removeEventListener("webkitfullscreenchange", sync);
    };
  }, [supported]);

  const toggle = useCallback(() => {
    // The browser may refuse (e.g. permissions policy); the change event keeps state truthful either way.
    Promise.resolve(currentFullscreenElement() ? exit() : enter()).catch(() => {});
  }, []);

  return { supported, active, toggle };
}
