import { describe, expect, it } from "vitest";
import { dueState, validateTitle, visibleTodos } from "./domain";
import type { Todo } from "./types";

const todo = (overrides: Partial<Todo>): Todo => ({
  id: "1", title: "task", completed: false, dueAt: null, priority: "none",
  sortOrder: 0, createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z",
  completedAt: null, ...overrides,
});

describe("todo domain", () => {
  it("validates trimmed titles", () => {
    expect(validateTitle("  ")).toBeTruthy();
    expect(validateTitle(" clear desk ")).toBeNull();
  });

  it("filters and sorts by priority", () => {
    const result = visibleTodos([
      todo({ id: "low", priority: "low" }),
      todo({ id: "done", completed: true, priority: "high" }),
      todo({ id: "high", priority: "high" }),
    ], "active", "priority");
    expect(result.map((item) => item.id)).toEqual(["high", "low"]);
  });

  it("classifies local due dates", () => {
    const now = new Date(2026, 9, 7, 12);
    expect(dueState("2026-10-06T23:59", now)).toBe("overdue");
    expect(dueState("2026-10-07T18:00", now)).toBe("today");
    expect(dueState(null, now)).toBe("none");
  });
});
