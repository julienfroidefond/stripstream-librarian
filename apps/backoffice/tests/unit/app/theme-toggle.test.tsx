import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const useTheme = vi.hoisted(() => vi.fn());

vi.mock("next-themes", () => ({ useTheme }));

import { ThemeSelector, ThemeToggle } from "@/app/theme-toggle";

const setTheme = vi.fn();

beforeEach(() => {
  setTheme.mockClear();
});

describe("ThemeToggle", () => {
  it("toggles from a dark resolved system theme to light", async () => {
    useTheme.mockReturnValue({ theme: "system", resolvedTheme: "dark", systemTheme: "dark", setTheme });
    render(<ThemeToggle />);
    const button = screen.getByRole("button", { name: "theme.switchToLight" });
    await userEvent.click(button);
    expect(setTheme).toHaveBeenCalledWith("light");
  });

  it("toggles from a light resolved system theme to dark", async () => {
    useTheme.mockReturnValue({ theme: "system", resolvedTheme: "light", systemTheme: "light", setTheme });
    render(<ThemeToggle />);
    const button = screen.getByRole("button", { name: "theme.switchToDark" });
    await userEvent.click(button);
    expect(setTheme).toHaveBeenCalledWith("dark");
  });

  it("toggles an explicit dark theme to light", async () => {
    useTheme.mockReturnValue({ theme: "dark", resolvedTheme: "dark", systemTheme: "light", setTheme });
    render(<ThemeToggle />);
    await userEvent.click(screen.getByRole("button", { name: "theme.switchToLight" }));
    expect(setTheme).toHaveBeenCalledWith("light");
  });

  it("toggles an explicit light theme to dark", async () => {
    useTheme.mockReturnValue({ theme: "light", resolvedTheme: "light", systemTheme: "dark", setTheme });
    render(<ThemeToggle />);
    await userEvent.click(screen.getByRole("button", { name: "theme.switchToDark" }));
    expect(setTheme).toHaveBeenCalledWith("dark");
  });
});

describe("ThemeSelector", () => {
  it("renders the three themes and highlights the active one", () => {
    useTheme.mockReturnValue({ theme: "dark", setTheme });
    render(<ThemeSelector />);
    expect(screen.getByTitle("theme.light")).toBeInTheDocument();
    expect(screen.getByTitle("theme.dark").className).toContain("bg-accent");
    expect(screen.getByTitle("theme.system")).toBeInTheDocument();
  });

  it("selects a theme on click", async () => {
    useTheme.mockReturnValue({ theme: "system", setTheme });
    render(<ThemeSelector />);
    await userEvent.click(screen.getByTitle("theme.light"));
    expect(setTheme).toHaveBeenCalledWith("light");
  });
});
