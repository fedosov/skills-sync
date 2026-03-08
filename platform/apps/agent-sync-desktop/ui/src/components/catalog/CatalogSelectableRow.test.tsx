import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { CatalogSelectableRow } from "./CatalogSelectableRow";

describe("CatalogSelectableRow", () => {
  it("renders shared row content and dispatches clicks", async () => {
    const onClick = vi.fn();

    render(
      <CatalogSelectableRow
        name="Project MCP"
        subtitle="exa"
        scope="project"
        selected={true}
        isFavorite={true}
        onClick={onClick}
        meta={<span>Archived</span>}
        footer={<span>Footer metadata</span>}
      />,
    );

    const button = screen.getByRole("button", { name: /Project MCP/i });
    await userEvent.click(button);

    expect(onClick).toHaveBeenCalledOnce();
    expect(button.className).toContain("bg-accent/85");
    expect(screen.getByText("Project")).toBeInTheDocument();
    expect(screen.getByText("exa")).toBeInTheDocument();
    expect(screen.getByText("Archived")).toBeInTheDocument();
    expect(screen.getByText("Footer metadata")).toBeInTheDocument();
  });

  it("renders the hover state when not selected", () => {
    render(
      <CatalogSelectableRow
        name="Global Skill"
        subtitle="global-skill"
        scope="global"
        selected={false}
        isFavorite={false}
        onClick={vi.fn()}
      />,
    );

    const button = screen.getByRole("button", { name: /Global Skill/i });
    expect(button.className).toContain("hover:bg-accent/55");
    expect(screen.getByText("Global")).toBeInTheDocument();
  });
});
