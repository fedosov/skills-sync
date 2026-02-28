import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AppFooter } from "./AppFooter";

describe("AppFooter", () => {
  it("shows manual mode when no next run exists", () => {
    render(
      <AppFooter
        nextRunAt={null}
        onRefreshNow={vi.fn()}
        refreshIntervalMinutes={0}
      />,
    );

    expect(screen.getByText(/Manual refresh mode/i)).toBeInTheDocument();
  });

  it("triggers refresh callback", async () => {
    const user = userEvent.setup();
    const onRefreshNow = vi.fn();
    render(
      <AppFooter
        nextRunAt={Date.now() + 60_000}
        onRefreshNow={onRefreshNow}
        refreshIntervalMinutes={5}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Refresh now" }));
    expect(onRefreshNow).toHaveBeenCalledTimes(1);
  });
});
