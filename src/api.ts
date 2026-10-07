import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, BoardState, DesktopPalette, InteractionMode, Todo, TodoInput, WidgetLayout } from "./types";

export const api = {
  listTodos: () => invoke<Todo[]>("list_todos"),
  createTodo: (input: TodoInput) => invoke<Todo>("create_todo", { input }),
  updateTodo: (id: string, input: TodoInput) => invoke<Todo>("update_todo", { id, input }),
  setTodoCompleted: (id: string, completed: boolean) =>
    invoke<Todo>("set_todo_completed", { id, completed }),
  deleteTodo: (id: string) => invoke<void>("delete_todo", { id }),
  reorderTodos: (orderedIds: string[]) => invoke<void>("reorder_todos", { orderedIds }),
  getBoardState: () => invoke<BoardState>("get_board_state"),
  saveWidgetLayout: (layout: WidgetLayout) => invoke<void>("save_widget_layout", { layout }),
  updateSettings: (settings: AppSettings) => invoke<AppSettings>("update_settings", { settings }),
  setInteractionMode: (mode: InteractionMode, layout: WidgetLayout | null) =>
    invoke<void>("set_interaction_mode", { mode, layout }),
  getDesktopPalette: () => invoke<DesktopPalette>("get_desktop_palette"),
};
