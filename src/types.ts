export type Priority = "none" | "low" | "medium" | "high";
export type TodoFilter = "active" | "completed" | "all";
export type TodoSort = "manual" | "due" | "priority";
export type InteractionMode = "locked" | "editing";
export type ThemePreference = "system" | "light" | "dark";

export interface Todo {
  id: string;
  title: string;
  completed: boolean;
  dueAt: string | null;
  priority: Priority;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  completedAt: string | null;
}

export interface TodoInput {
  title: string;
  dueAt: string | null;
  priority: Priority;
}

export interface WidgetLayout {
  id: string;
  kind: "todo-list";
  x: number;
  y: number;
  width: number;
  height: number;
  settings: Record<string, unknown>;
}

export interface AppSettings {
  autoStart: boolean;
  opacity: number;
  shadowStrength: number;
  theme: ThemePreference;
}

export interface BoardState {
  widgets: WidgetLayout[];
  settings: AppSettings;
}

export interface DesktopPalette {
  surface: "light" | "dark";
  wallpaperLuminance: number;
}
