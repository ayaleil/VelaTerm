import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("./transport", () => ({ invoke }));

import { cloudflareAccessStart } from "./cloudflareAccess";

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

describe("cloudflareAccessStart", () => {
  it("sends the backend command name with the exact argument shape", async () => {
    await cloudflareAccessStart({
      teamDomain: "myteam.cloudflareaccess.com",
      aud: "tag",
      port: 8800,
      allowedEmails: ["a@x.com"],
      password: "pw",
    });
    expect(invoke).toHaveBeenCalledWith("cloudflare_access_start", {
      teamDomain: "myteam.cloudflareaccess.com",
      aud: "tag",
      port: 8800,
      allowedEmails: ["a@x.com"],
      password: "pw",
    });
  });

  it("sends null for an empty password so the backend reuses the stored one", async () => {
    await cloudflareAccessStart({
      teamDomain: "myteam.cloudflareaccess.com",
      aud: "tag",
      port: 8800,
      allowedEmails: [],
      password: "",
    });
    expect(invoke).toHaveBeenCalledWith(
      "cloudflare_access_start",
      expect.objectContaining({ password: null }),
    );
  });
});
