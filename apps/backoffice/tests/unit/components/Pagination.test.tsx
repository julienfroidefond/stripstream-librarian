import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { OffsetPagination } from "@/app/components/ui/Pagination";

let pushState: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  pushState = vi.spyOn(window.history, "pushState").mockImplementation(() => {});
});

function renderPagination(props: Partial<Parameters<typeof OffsetPagination>[0]> = {}) {
  return render(
    <OffsetPagination currentPage={2} totalPages={3} pageSize={24} totalItems={60} {...props} />
  );
}

describe("OffsetPagination", () => {
  it("renders the item range and page size options", () => {
    renderPagination();
    expect(screen.getByText(/pagination\.range/)).toBeInTheDocument();
    expect(screen.getByText(/pagination\.range/).textContent).toContain("25");
    expect(screen.getByRole("combobox")).toHaveValue("24");
    expect(screen.getByRole("option", { name: "48" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "96" })).toBeInTheDocument();
  });

  it("navigates to the previous and next page", () => {
    renderPagination();

    fireEvent.click(screen.getByTitle("common.nextPage"));
    expect(pushState).toHaveBeenLastCalledWith(null, "", "?page=3");

    fireEvent.click(screen.getByTitle("common.previousPage"));
    expect(pushState).toHaveBeenLastCalledWith(null, "", "?page=1");
  });

  it("navigates when clicking a page number", () => {
    renderPagination();
    fireEvent.click(screen.getByRole("button", { name: "1" }));
    expect(pushState).toHaveBeenLastCalledWith(null, "", "?page=1");
  });

  it("resets to page 1 when changing the page size", () => {
    renderPagination();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "48" } });
    expect(pushState).toHaveBeenLastCalledWith(null, "", "?limit=48&page=1");
  });

  it("disables previous on the first page", () => {
    renderPagination({ currentPage: 1 });
    expect(screen.getByTitle("common.previousPage")).toBeDisabled();
  });

  it("disables next on the last page", () => {
    renderPagination({ currentPage: 3 });
    expect(screen.getByTitle("common.nextPage")).toBeDisabled();
  });

  it("shows an ellipsis when there are many pages", () => {
    renderPagination({ currentPage: 5, totalPages: 20 });
    expect(screen.getAllByText("...").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "4" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "6" })).toBeInTheDocument();
  });

  it("shows all pages when they fit", () => {
    renderPagination({ currentPage: 2, totalPages: 4 });
    expect(screen.queryByText("...")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "4" })).toBeInTheDocument();
  });
});
