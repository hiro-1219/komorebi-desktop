import type { Priority, Todo, TodoFilter, TodoSort } from "./types";

const priorityRank: Record<Priority, number> = { none: 0, low: 1, medium: 2, high: 3 };

export function validateTitle(value: string): string | null {
  const title = value.trim();
  if (!title) return "タイトルを入力してください";
  if ([...title].length > 200) return "タイトルは200文字以内で入力してください";
  return null;
}

export function visibleTodos(todos: Todo[], filter: TodoFilter, sort: TodoSort): Todo[] {
  const filtered = todos.filter((todo) =>
    filter === "all" ? true : filter === "completed" ? todo.completed : !todo.completed,
  );
  return [...filtered].sort((a, b) => {
    if (sort === "priority") return priorityRank[b.priority] - priorityRank[a.priority] || a.sortOrder - b.sortOrder;
    if (sort === "due") {
      if (!a.dueAt && !b.dueAt) return a.sortOrder - b.sortOrder;
      if (!a.dueAt) return 1;
      if (!b.dueAt) return -1;
      return a.dueAt.localeCompare(b.dueAt) || a.sortOrder - b.sortOrder;
    }
    return a.sortOrder - b.sortOrder;
  });
}

export function dueState(dueAt: string | null, now = new Date()): "none" | "today" | "overdue" | "future" {
  if (!dueAt) return "none";
  const due = new Date(dueAt);
  if (Number.isNaN(due.getTime())) return "none";
  if (due.getTime() < now.getTime()) return "overdue";
  if (due.getFullYear() === now.getFullYear() && due.getMonth() === now.getMonth() && due.getDate() === now.getDate()) return "today";
  return "future";
}

export function formatDueAt(dueAt: string): string {
  const due = new Date(dueAt);
  if (Number.isNaN(due.getTime())) return dueAt;
  return new Intl.DateTimeFormat("ja-JP", {
    month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit",
  }).format(due);
}
