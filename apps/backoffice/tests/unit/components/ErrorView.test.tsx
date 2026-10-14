import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ErrorView } from "@/app/components/ErrorView";

describe("ErrorView", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("shows the error copy and reports the error", () => {
    const error = new Error("boom");
    render(<ErrorView error={error} reset={vi.fn()} />);

    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("error.title");
    expect(screen.getByText("error.description")).toBeInTheDocument();
    expect(console.error).toHaveBeenCalledWith(error);
  });

  it("calls reset when the user retries", () => {
    const reset = vi.fn();
    render(<ErrorView error={new Error("boom")} reset={reset} />);

    fireEvent.click(screen.getByText("error.retry"));

    expect(reset).toHaveBeenCalledTimes(1);
  });

  it("shows a reference and a home link when a digest is available", () => {
    const error = Object.assign(new Error("boom"), { digest: "abc123" });
    render(<ErrorView error={error} reset={vi.fn()} />);

    expect(screen.getByText(/^error\.reference/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "error.backHome" })).toHaveAttribute("href", "/");
  });

  it("omits the reference when there is no digest", () => {
    render(<ErrorView error={new Error("boom")} reset={vi.fn()} />);

    expect(screen.queryByText(/^error\.reference/)).not.toBeInTheDocument();
  });
});
