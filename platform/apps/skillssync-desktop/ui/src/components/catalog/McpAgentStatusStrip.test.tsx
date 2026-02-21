import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { McpAgentStatusStrip } from "./McpAgentStatusStrip";

describe("McpAgentStatusStrip", () => {
  it("renders per-agent status chips for project scope", () => {
    render(
      <McpAgentStatusStrip
        scope="project"
        enabledByAgent={{
          codex: true,
          claude: false,
          project: true,
        }}
      />,
    );

    expect(
      screen.getByRole("img", { name: "codex enabled" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "project enabled" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "claude disabled" }),
    ).toBeInTheDocument();
    expect(screen.getAllByText("ON")).toHaveLength(2);
    expect(screen.getAllByText("OFF")).toHaveLength(1);
  });

  it("does not render project agent for global scope", () => {
    render(
      <McpAgentStatusStrip
        scope="global"
        enabledByAgent={{
          codex: false,
          claude: true,
          project: true,
        }}
      />,
    );

    expect(
      screen.queryByRole("img", { name: /project (enabled|disabled)/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "codex disabled" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "claude enabled" }),
    ).toBeInTheDocument();
  });

  it("renders OFF mini-label for every disabled agent", () => {
    render(
      <McpAgentStatusStrip
        scope="project"
        enabledByAgent={{
          codex: false,
          claude: false,
          project: false,
        }}
      />,
    );

    expect(screen.queryByText("ON")).not.toBeInTheDocument();
    expect(screen.getAllByText("OFF")).toHaveLength(3);
    expect(
      screen.getByRole("img", { name: "codex disabled" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "claude disabled" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "project disabled" }),
    ).toBeInTheDocument();
  });
});
