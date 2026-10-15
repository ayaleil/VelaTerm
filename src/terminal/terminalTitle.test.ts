import { describe, expect, it } from "vitest";
import { sessionNameFromTitle } from "./terminalTitle";

describe("sessionNameFromTitle", () => {
  it("uses the title a Claude session sets", () => {
    expect(sessionNameFromTitle("claude", "Fix login bug")).toBe("Fix login bug");
  });

  it("strips Claude's status glyph prefix", () => {
    expect(sessionNameFromTitle("claude", "✳ Fix login bug")).toBe("Fix login bug");
    expect(sessionNameFromTitle("claude", "⠂ Fix login bug")).toBe("Fix login bug");
  });

  it("ignores Claude's default title", () => {
    expect(sessionNameFromTitle("claude", "✳ Claude Code")).toBeNull();
  });

  it("ignores empty titles", () => {
    expect(sessionNameFromTitle("claude", "  ✳  ")).toBeNull();
  });

  it("ignores titles from non-Claude sessions", () => {
    expect(sessionNameFromTitle("terminal", "user@host: ~/project")).toBeNull();
    expect(sessionNameFromTitle("codex", "Fix login bug")).toBeNull();
  });

  it("caps very long titles", () => {
    expect(sessionNameFromTitle("claude", "a".repeat(200))).toHaveLength(80);
  });
});
