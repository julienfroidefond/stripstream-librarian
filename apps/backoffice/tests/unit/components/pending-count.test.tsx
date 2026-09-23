import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const linkStatus = { pending: false };

vi.mock("next/link", async () => {
  const React = await import("react");
  return {
    __esModule: true,
    default: ({ href, children, ...rest }: any) =>
      React.createElement(
        "a",
        { href: typeof href === "string" ? href : (href?.pathname ?? "#"), ...rest },
        children
      ),
    useLinkStatus: () => linkStatus,
  };
});

import { PendingCount } from "@/app/components/ui";

describe("PendingCount", () => {
  beforeEach(() => {
    linkStatus.pending = false;
  });

  it("renders the count while the link is idle", () => {
    render(<PendingCount count={42} />);
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.queryByRole("img", { hidden: true })).not.toBeInTheDocument();
  });

  it("swaps the count for a spinner while the link is pending", () => {
    linkStatus.pending = true;
    const { container } = render(<PendingCount count={42} />);
    expect(screen.queryByText("42")).not.toBeInTheDocument();
    expect(container.querySelector("svg")).toBeTruthy();
  });

  it("renders a zero count instead of hiding the chip", () => {
    render(<PendingCount count={0} />);
    expect(screen.getByText("0")).toBeInTheDocument();
  });
});
