//! The fullscreen toggle follows the document's real state, falls back to the webkit-prefixed API, and reports
//! itself unsupported when neither API exists.

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useFullscreen } from "./useFullscreen";

function Toggle() {
  const { supported, active, toggle } = useFullscreen();
  return (
    <button data-testid="toggle" data-supported={String(supported)} onClick={toggle}>
      {active ? "active" : "inactive"}
    </button>
  );
}

let fullscreenElement: Element | null = null;

function stub(target: object, key: string, value: unknown) {
  Object.defineProperty(target, key, { configurable: true, value });
}

function unstub(target: object, ...keys: string[]) {
  for (const key of keys) Reflect.deleteProperty(target, key);
}

function setFullscreen(el: Element | null, eventName: string) {
  fullscreenElement = el;
  document.dispatchEvent(new Event(eventName));
}

function installStandard() {
  const request = vi.fn(async () => setFullscreen(document.documentElement, "fullscreenchange"));
  const exit = vi.fn(async () => setFullscreen(null, "fullscreenchange"));
  stub(document, "fullscreenEnabled", true);
  Object.defineProperty(document, "fullscreenElement", { configurable: true, get: () => fullscreenElement });
  stub(document, "exitFullscreen", exit);
  stub(document.documentElement, "requestFullscreen", request);
  return { request, exit };
}

function installWebkit() {
  const request = vi.fn(() => setFullscreen(document.documentElement, "webkitfullscreenchange"));
  const exit = vi.fn(() => setFullscreen(null, "webkitfullscreenchange"));
  stub(document, "webkitFullscreenEnabled", true);
  Object.defineProperty(document, "webkitFullscreenElement", { configurable: true, get: () => fullscreenElement });
  stub(document, "webkitExitFullscreen", exit);
  stub(document.documentElement, "webkitRequestFullscreen", request);
  return { request, exit };
}

afterEach(() => {
  cleanup();
  fullscreenElement = null;
  unstub(document, "fullscreenEnabled", "fullscreenElement", "exitFullscreen", "webkitFullscreenEnabled", "webkitFullscreenElement", "webkitExitFullscreen");
  unstub(document.documentElement, "requestFullscreen", "webkitRequestFullscreen");
});

describe("useFullscreen", () => {
  it("is unsupported when the browser exposes no Fullscreen API", () => {
    render(<Toggle />);
    expect(screen.getByTestId("toggle").dataset.supported).toBe("false");
  });

  it("enters and exits through the standard API", async () => {
    const { request, exit } = installStandard();
    render(<Toggle />);
    const button = screen.getByTestId("toggle");
    expect(button.dataset.supported).toBe("true");

    await act(async () => fireEvent.click(button));
    expect(request).toHaveBeenCalledTimes(1);
    expect(button.textContent).toBe("active");

    await act(async () => fireEvent.click(button));
    expect(exit).toHaveBeenCalledTimes(1);
    expect(button.textContent).toBe("inactive");
  });

  it("falls back to the webkit-prefixed API", async () => {
    const { request, exit } = installWebkit();
    render(<Toggle />);
    const button = screen.getByTestId("toggle");

    await act(async () => fireEvent.click(button));
    expect(request).toHaveBeenCalledTimes(1);
    expect(button.textContent).toBe("active");

    await act(async () => fireEvent.click(button));
    expect(exit).toHaveBeenCalledTimes(1);
    expect(button.textContent).toBe("inactive");
  });

  it("tracks exits the user makes outside the button, such as Esc", () => {
    installStandard();
    render(<Toggle />);
    const button = screen.getByTestId("toggle");

    act(() => setFullscreen(document.documentElement, "fullscreenchange"));
    expect(button.textContent).toBe("active");
    act(() => setFullscreen(null, "fullscreenchange"));
    expect(button.textContent).toBe("inactive");
  });

  it("swallows a refused request without changing state", async () => {
    installStandard();
    stub(document.documentElement, "requestFullscreen", vi.fn(() => Promise.reject(new TypeError("denied"))));
    render(<Toggle />);
    const button = screen.getByTestId("toggle");

    await act(async () => fireEvent.click(button));
    expect(button.textContent).toBe("inactive");
  });
});
