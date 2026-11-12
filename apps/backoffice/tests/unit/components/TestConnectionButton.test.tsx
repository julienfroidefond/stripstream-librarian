import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TestConnectionButton } from "@/app/(app)/settings/components/TestConnectionButton";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("TestConnectionButton", () => {
  it("calls the endpoint and shows the success message", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue({ json: async () => ({ success: true, message: "Connected" }) });
    vi.stubGlobal("fetch", fetchMock);
    render(<TestConnectionButton target="prowlarr" />);

    fireEvent.click(screen.getByRole("button", { name: "settings.testConnection" }));

    expect(fetchMock.mock.calls[0][0]).toBe("/api/prowlarr/test");
    expect(await screen.findByText("Connected")).toBeInTheDocument();
  });

  it("shows the error returned by the endpoint", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ json: async () => ({ error: "Bad credentials" }) })
    );
    render(<TestConnectionButton target="qbittorrent" />);

    fireEvent.click(screen.getByRole("button"));

    expect(await screen.findByText("Bad credentials")).toBeInTheDocument();
  });

  it("shows a generic message when the request fails", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("network")));
    render(<TestConnectionButton target="qbittorrent" />);

    fireEvent.click(screen.getByRole("button"));

    expect(await screen.findByText("Failed to connect")).toBeInTheDocument();
  });

  it("shows a spinner while testing", async () => {
    vi.stubGlobal("fetch", vi.fn().mockReturnValue(new Promise(() => {})));
    render(<TestConnectionButton target="qbittorrent" />);

    fireEvent.click(screen.getByRole("button"));

    expect(screen.getByRole("button", { name: /settings.testing/ })).toBeDisabled();
  });

  it("stays disabled when the disabled prop is set", () => {
    vi.stubGlobal("fetch", vi.fn());
    render(<TestConnectionButton target="qbittorrent" disabled />);

    expect(screen.getByRole("button")).toBeDisabled();
  });
});
