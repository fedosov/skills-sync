import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AppHeader } from "./AppHeader";

describe("AppHeader", () => {
  it("shows selected refresh interval and updates it", async () => {
    const user = userEvent.setup();
    const onRefreshIntervalChange = vi.fn();

    render(
      <AppHeader
        syncStatus="ok"
        activeSkillCount={1}
        archivedSkillCount={0}
        totalSkills={1}
        activeSubagentCount={0}
        mcpCount={0}
        query=""
        onQueryChange={vi.fn()}
        busy={false}
        allowFilesystemChanges={true}
        onAllowFilesystemChangesToggle={vi.fn()}
        onSync={vi.fn()}
        onOpenAuditLog={vi.fn()}
        refreshIntervalMinutes={15}
        onRefreshIntervalChange={onRefreshIntervalChange}
      />,
    );

    const select = screen.getByLabelText("Auto refresh interval");
    expect(select).toHaveValue("15");

    await user.selectOptions(select, "5");
    expect(onRefreshIntervalChange).toHaveBeenCalledWith(5);
  });
});
