import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import RatingStars from "@/app/components/RatingStars";

function starSpans(): HTMLElement[] {
  const slider = screen.getByRole("slider");
  return Array.from(slider.querySelectorAll("span"));
}

describe("RatingStars", () => {
  it("renders a slider with an aria label derived from the value", () => {
    render(<RatingStars value={7} />);
    expect(screen.getByRole("slider")).toHaveAttribute("aria-label", "3.5 sur 5");
    expect(screen.getByRole("slider")).toHaveAttribute("aria-valuenow", "7");
    expect(screen.getByRole("slider")).toHaveAttribute("aria-valuemax", "10");
  });

  it("labels an unrated control", () => {
    render(<RatingStars value={null} />);
    expect(screen.getByRole("slider")).toHaveAttribute("aria-label", "Non noté");
  });

  it("renders a read-only image without a slider", () => {
    render(<RatingStars value={4} readOnly />);
    expect(screen.queryByRole("slider")).not.toBeInTheDocument();
    expect(screen.getByRole("img")).toBeInTheDocument();
  });

  it("emits a full star value when clicking the right half", () => {
    const onChange = vi.fn();
    render(<RatingStars value={null} onChange={onChange} />);

    fireEvent.click(starSpans()[0], { clientX: 100 });

    expect(onChange).toHaveBeenCalledWith(2);
  });

  it("emits a half star value when clicking the left half", () => {
    const onChange = vi.fn();
    render(<RatingStars value={null} onChange={onChange} />);

    fireEvent.click(starSpans()[2], { clientX: -100 });

    expect(onChange).toHaveBeenCalledWith(5);
  });

  it("does not emit when read-only", () => {
    const onChange = vi.fn();
    render(<RatingStars value={null} readOnly onChange={onChange} />);

    const stars = screen.getByRole("img").querySelectorAll("span");
    fireEvent.click(stars[0], { clientX: 100 });

    expect(onChange).not.toHaveBeenCalled();
  });

  it("shows a clear button only for a rated, editable control", () => {
    const onClear = vi.fn();
    const { rerender } = render(<RatingStars value={6} onClear={onClear} />);
    fireEvent.click(screen.getByRole("button", { name: "Supprimer la note" }));
    expect(onClear).toHaveBeenCalledTimes(1);

    rerender(<RatingStars value={null} onClear={onClear} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();

    rerender(<RatingStars value={6} readOnly onClear={onClear} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
