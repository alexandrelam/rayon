// @vitest-environment jsdom

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LauncherArgumentPanel } from "./LauncherArgumentPanel";
import { LauncherFooter } from "./LauncherFooter";
import { LauncherResultItem } from "./LauncherResultItem";

describe("Launcher overflow handling", () => {
  it("keeps long result content width-safe", () => {
    render(
      <ul>
        <LauncherResultItem
          item={{
            id: "github.my-prs#123",
            title:
              "Very long pull request title that should truncate before it can stretch the launcher shell width",
            meta: "feature/super-long-branch-name-with-no-natural-breaks-and-a-lot-of-identifiers-1234567890abcdefghijklmnopqrstuvwxyz",
            kind: "github-pull-request-result",
            selected: false,
          }}
          onSelect={vi.fn()}
          setRef={vi.fn()}
        />
      </ul>,
    );

    const button = screen.getByRole("button");
    expect(button.className).toContain("min-w-0");
    expect(button.className).toContain("whitespace-normal");

    expect(screen.getByText(/Very long pull request title/).className).toContain("truncate");
    expect(screen.getByText(/feature\/super-long-branch-name/).className).toContain(
      "[overflow-wrap:anywhere]",
    );
    expect(screen.getByText(/github-pull-request-result/).className).toContain("shrink-0");
  });

  it("wraps long footer and argument panel content inside the panel", () => {
    render(
      <>
        <LauncherFooter
          message="https://example.com/this/is/a/very/long/footer/message/that/should/wrap/instead/of/pushing/the/container/beyond/its/padding"
          error="error:error:error:error:error:error:error:error:error:error"
          muted={false}
        />
        <LauncherArgumentPanel
          currentStep={1}
          totalSteps={2}
          flagLabel="--extremely-long-flag-name-that-should-wrap-inside-the-panel-without-breaking-the-layout"
          defaultValue="default-value-with-no-breakpoints-abcdefghijklmnopqrstuvwxyz0123456789"
        />
      </>,
    );

    expect(
      screen
        .getByText(/https:\/\/example.com\/this\/is\/a\/very\/long\/footer\/message/)
        .className,
    ).toContain("[overflow-wrap:anywhere]");
    expect(screen.getByText(/error:error:error/).className).toContain("[overflow-wrap:anywhere]");
    expect(screen.getByText(/--extremely-long-flag-name/).className).toContain(
      "[overflow-wrap:anywhere]",
    );
    expect(screen.getByText(/default-value-with-no-breakpoints/).className).toContain(
      "[overflow-wrap:anywhere]",
    );
  });
});
