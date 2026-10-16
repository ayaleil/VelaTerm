import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const envMock = vi.hoisted(() => ({ env: { isTauri: true, isElectron: false } }));
vi.mock("../platform/env", () => envMock);
vi.mock("../ipc/transport", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

import { invoke } from "../ipc/transport";
import { setLang, t } from "../i18n";
import { useNativeDialogCopy } from "./useNativeDialogCopy";

beforeEach(() => {
  envMock.env.isTauri = true;
  envMock.env.isElectron = false;
  setLang("en");
  vi.clearAllMocks();
});
afterEach(cleanup);

it("pushes the localized hidden-mode copy to the backend", () => {
  setLang("es");
  renderHook(() => useNativeDialogCopy());
  expect(invoke).toHaveBeenCalledWith("set_hidden_mode_copy", {
    copy: expect.objectContaining({
      hidePrompt: t("hiddenMode.hidePrompt"),
      returnToLocal: t("hiddenMode.returnToLocal"),
      quit: t("hiddenMode.quit"),
    }),
  });
});

it("does nothing in remote windows (browser transport)", () => {
  envMock.env.isTauri = false;
  renderHook(() => useNativeDialogCopy());
  expect(invoke).not.toHaveBeenCalled();
});

it("does nothing under Electron", () => {
  envMock.env.isElectron = true;
  renderHook(() => useNativeDialogCopy());
  expect(invoke).not.toHaveBeenCalled();
});
