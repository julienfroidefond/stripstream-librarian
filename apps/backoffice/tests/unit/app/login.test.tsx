import { render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import LoginPage from "@/app/login/page";

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  Object.defineProperty(window, "location", {
    configurable: true,
    value: { href: "", assign: vi.fn(), reload: vi.fn() },
  });
  fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

async function fillAndSubmit() {
  const user = userEvent.setup();
  await user.type(screen.getByLabelText("Identifiant"), "admin");
  await user.type(screen.getByLabelText("Mot de passe"), "secret");
  await user.click(screen.getByRole("button", { name: "Se connecter" }));
}

describe("LoginPage", () => {
  it("renders the hero and the credential fields", () => {
    render(<LoginPage />);

    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("StripStream");
    expect(screen.getByLabelText("Identifiant")).toBeInTheDocument();
    expect(screen.getByLabelText("Mot de passe")).toBeInTheDocument();
  });

  it("posts the credentials and redirects on success", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => ({}) });
    render(<LoginPage />);

    await fillAndSubmit();

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/auth/login");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body as string)).toEqual({ username: "admin", password: "secret" });
    expect(window.location.href).toBe("/");
  });

  it("shows the API error message on failure", async () => {
    fetchMock.mockResolvedValue({ ok: false, json: async () => ({ error: "Bad credentials" }) });
    render(<LoginPage />);

    await fillAndSubmit();

    expect(await screen.findByText("Bad credentials")).toBeInTheDocument();
  });

  it("falls back to a generic message when the body is not JSON", async () => {
    fetchMock.mockResolvedValue({
      ok: false,
      json: async () => {
        throw new Error("invalid json");
      },
    });
    render(<LoginPage />);

    await fillAndSubmit();

    expect(await screen.findByText("Identifiants invalides")).toBeInTheDocument();
  });

  it("reports a network error", async () => {
    fetchMock.mockRejectedValue(new Error("offline"));
    render(<LoginPage />);

    await fillAndSubmit();

    expect(await screen.findByText("Erreur réseau")).toBeInTheDocument();
  });
});
