import { describe, expect, it } from "vitest";
import { applyThemePreference } from "./theme";

describe("theme helpers", () => {
  it("applies the selected theme to the root element", () => {
    Object.defineProperty(globalThis, "document", {
      value: {
        documentElement: {
          dataset: {},
        },
      },
      configurable: true,
    });

    applyThemePreference("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");

    applyThemePreference("system");
    expect(document.documentElement.dataset.theme).toBe("system");
  });
});
