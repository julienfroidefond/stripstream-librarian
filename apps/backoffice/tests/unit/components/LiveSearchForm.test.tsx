import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LiveSearchForm } from "@/app/components/LiveSearchForm";

const fields = [
  { name: "q", type: "text" as const, label: "common.search", placeholder: "series.searchPlaceholder" },
  {
    name: "library",
    type: "select" as const,
    label: "books.library",
    options: [
      { value: "", label: "books.allLibraries" },
      { value: "lib-1", label: "Main library" },
    ],
  },
  {
    name: "sort",
    type: "select" as const,
    label: "books.sort",
    options: [
      { value: "", label: "books.sortTitle" },
      { value: "latest", label: "books.sortLatest" },
    ],
  },
];

let replaceState: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  replaceState = vi.spyOn(window.history, "replaceState").mockImplementation(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  replaceState.mockRestore();
});

describe("LiveSearchForm", () => {
  it("renders one search field and one pill per select, with the current values", () => {
    render(
      <LiveSearchForm
        basePath="/series"
        fields={fields}
        initialValues={{ q: "naruto", library: "lib-1" }}
      />,
    );

    expect(screen.getByRole("textbox")).toHaveValue("naruto");
    expect(screen.getByTitle("books.library: Main library")).toBeInTheDocument();
    expect(screen.getByTitle("books.sort: books.sortTitle")).toBeInTheDocument();
    expect(screen.getByLabelText("books.library")).toHaveValue("lib-1");
    expect(screen.getByLabelText("books.sort")).toHaveValue("");
  });

  it("navigates immediately when a filter changes", () => {
    render(<LiveSearchForm basePath="/series" fields={fields} />);

    fireEvent.change(screen.getByLabelText("books.library"), { target: { value: "lib-1" } });

    expect(replaceState).toHaveBeenLastCalledWith(null, "", "/series?library=lib-1");
  });

  it("keeps hidden values in the generated URL", () => {
    render(
      <LiveSearchForm
        basePath="/metadata"
        fields={fields}
        hiddenValues={{ tab: "books" }}
        initialValues={{ q: "", library: "" }}
      />,
    );

    fireEvent.change(screen.getByLabelText("books.library"), { target: { value: "lib-1" } });

    expect(replaceState).toHaveBeenLastCalledWith(null, "", "/metadata?tab=books&library=lib-1");
  });

  it("debounces text input before navigating", () => {
    vi.useFakeTimers();
    render(<LiveSearchForm basePath="/series" fields={fields} debounceMs={300} />);

    fireEvent.change(screen.getByRole("textbox"), { target: { value: "a" } });
    expect(replaceState).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(replaceState).toHaveBeenLastCalledWith(null, "", "/series?q=a");
  });

  it("clears the search field without touching the filters", () => {
    render(
      <LiveSearchForm
        basePath="/series"
        fields={fields}
        initialValues={{ q: "naruto", library: "lib-1" }}
      />,
    );

    // First visible "clear" control is the one inside the search input.
    fireEvent.click(screen.getAllByRole("button", { name: "common.clear" })[0]);

    expect(screen.getByRole("textbox")).toHaveValue("");
    expect(screen.getByLabelText("books.library")).toHaveValue("lib-1");
    expect(replaceState).toHaveBeenLastCalledWith(null, "", "/series?library=lib-1");
  });

  it("resets every filter with the clear-all button, keeping hidden values", () => {
    render(
      <LiveSearchForm
        basePath="/metadata"
        fields={fields}
        hiddenValues={{ tab: "books" }}
        initialValues={{ q: "", library: "lib-1", sort: "latest" }}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "common.clear" }));

    expect(screen.getByLabelText("books.library")).toHaveValue("");
    expect(screen.getByLabelText("books.sort")).toHaveValue("");
    expect(replaceState).toHaveBeenLastCalledWith(null, "", "/metadata?tab=books");
  });

  it("counts active select filters on the mobile toggle", () => {
    render(
      <LiveSearchForm
        basePath="/series"
        fields={fields}
        initialValues={{ q: "naruto", library: "lib-1" }}
      />,
    );

    const toggle = screen.getByRole("checkbox", { name: "common.filters" });
    expect(toggle).not.toBeChecked();
    const label = screen.getByText(/common\.filters/);
    expect(within(label).getByText("1")).toBeInTheDocument();

    fireEvent.click(toggle);
    expect(toggle).toBeChecked();
  });
});
