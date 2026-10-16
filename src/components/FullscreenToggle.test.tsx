//! The fullscreen toggle appears only for plain browser access with a usable Fullscreen API; every desktop
//! shell (Tauri, Electron, remote-connection windows) hides it.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  env: { isBrowser: true, isTauri: false, isElectron: false, isRemoteWindow: false },
  fullscreen: { supported: true, active: false, toggle: vi.fn() },
}));

vi.mock("../platform/env", () => ({ env: mocks.env }));
vi.mock("../hooks/useFullscreen", () => ({ useFullscreen: () => mocks.fullscreen }));

import { FullscreenToggle } from "./FullscreenToggle";

beforeEach(() => {
  Object.assign(mocks.env, { isBrowser: true, isTauri: false, isElectron: false, isRemoteWindow: false });
  Object.assign(mocks.fullscreen, { supported: true, active: false });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("FullscreenToggle", () => {
  it("shows in a plain browser with the Fullscreen API and toggles on click", () => {
    render(<FullscreenToggle className="tb-btn" />);
    const button = screen.getByRole("button", { name: "Enter full screen" });
    fireEvent.click(button);
    expect(mocks.fullscreen.toggle).toHaveBeenCalledTimes(1);
  });

  it("switches its label while fullscreen is active", () => {
    mocks.fullscreen.active = true;
    render(<FullscreenToggle className="tb-btn" />);
    expect(screen.getByRole("button", { name: "Exit full screen" }).getAttribute("aria-pressed")).toBe("true");
  });

  it.each([
    ["Tauri", { isBrowser: false, isTauri: true }],
    ["Electron", { isBrowser: false, isElectron: true }],
    ["a remote-connection window", { isRemoteWindow: true }],
  ])("is hidden in %s", (_, overrides) => {
    Object.assign(mocks.env, overrides);
    render(<FullscreenToggle className="tb-btn" />);
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("is hidden when the browser has no Fullscreen API", () => {
    mocks.fullscreen.supported = false;
    render(<FullscreenToggle className="tb-btn" />);
    expect(screen.queryByRole("button")).toBeNull();
  });
});
