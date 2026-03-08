import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { EntityActionMenus } from "./EntityActionMenus";

describe("EntityActionMenus", () => {
  it("renders open and actions menus with configured items", async () => {
    const toggleOpen = vi.fn();
    const toggleActions = vi.fn();
    const openFolder = vi.fn();
    const archive = vi.fn();
    const remove = vi.fn();

    render(
      <EntityActionMenus
        openMenuExpanded={true}
        actionsMenuExpanded={true}
        onToggleOpenMenu={toggleOpen}
        onToggleActionsMenu={toggleActions}
        openItems={[
          { label: "Open folder", onSelect: openFolder },
          { label: "Open file", onSelect: vi.fn(), disabled: true },
        ]}
        actionItems={[
          { label: "Archive", onSelect: archive },
          { label: "Delete", onSelect: remove, tone: "destructive" },
        ]}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "Open…" }));
    await userEvent.click(screen.getByRole("button", { name: "More actions" }));
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Open folder" }),
    );
    await userEvent.click(screen.getByRole("menuitem", { name: "Archive" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Delete" }));

    expect(toggleOpen).toHaveBeenCalledOnce();
    expect(toggleActions).toHaveBeenCalledOnce();
    expect(openFolder).toHaveBeenCalledOnce();
    expect(archive).toHaveBeenCalledOnce();
    expect(remove).toHaveBeenCalledOnce();
    expect(screen.getByRole("menuitem", { name: "Open file" })).toBeDisabled();
  });

  it("omits the open menu trigger when no open items are provided", () => {
    render(
      <EntityActionMenus
        actionsMenuExpanded={false}
        onToggleActionsMenu={vi.fn()}
        actionItems={[{ label: "Restore", onSelect: vi.fn() }]}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Open…" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "More actions" }),
    ).toBeInTheDocument();
  });
});
