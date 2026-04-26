import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("theme stylesheet", () => {
  it("defines explicit dark and system theme branches", () => {
    const stylesheet = readFileSync(resolve(__dirname, "./styles/globals.css"), "utf8");

    expect(stylesheet).toContain(':root[data-theme="dark"]');
    expect(stylesheet).toContain(':root[data-theme="system"]');
    expect(stylesheet).toContain("@media (prefers-color-scheme: dark)");
    expect(stylesheet).toContain("--panel-foreground: rgba(244, 247, 252, 0.96);");
  });
});
