import { create } from "zustand";
import { api } from "./api";
import type { AppSettings, InteractionMode, Todo, TodoInput, WidgetLayout } from "./types";

interface AppStore {
  todos: Todo[];
  layout: WidgetLayout | null;
  settings: AppSettings;
  interactionMode: InteractionMode;
  loading: boolean;
  error: string | null;
  initialize: () => Promise<void>;
  addTodo: (input: TodoInput) => Promise<void>;
  updateTodo: (id: string, input: TodoInput) => Promise<void>;
  toggleTodo: (id: string, completed: boolean) => Promise<void>;
  removeTodo: (id: string) => Promise<void>;
  reorder: (orderedIds: string[]) => Promise<void>;
  saveLayout: (layout: WidgetLayout) => Promise<void>;
  saveSettings: (settings: AppSettings) => Promise<void>;
  setMode: (mode: InteractionMode, notifyBackend?: boolean) => Promise<void>;
  clearError: () => void;
}

const message = (error: unknown) => error instanceof Error ? error.message : String(error);

export const useAppStore = create<AppStore>((set, get) => ({
  todos: [],
  layout: null,
  settings: { autoStart: false, opacity: 0.7, shadowStrength: 0, theme: "system" },
  interactionMode: "locked",
  loading: true,
  error: null,

  initialize: async () => {
    try {
      const [todos, board] = await Promise.all([api.listTodos(), api.getBoardState()]);
      set({ todos, layout: board.widgets[0] ?? null, settings: board.settings, loading: false });
    } catch (error) {
      set({ error: message(error), loading: false });
    }
  },
  addTodo: async (input) => {
    try {
      const todo = await api.createTodo(input);
      set((state) => ({ todos: [...state.todos, todo], error: null }));
    } catch (error) { set({ error: message(error) }); throw error; }
  },
  updateTodo: async (id, input) => {
    try {
      const todo = await api.updateTodo(id, input);
      set((state) => ({ todos: state.todos.map((item) => item.id === id ? todo : item), error: null }));
    } catch (error) { set({ error: message(error) }); throw error; }
  },
  toggleTodo: async (id, completed) => {
    const previous = get().todos;
    set({ todos: previous.map((item) => item.id === id ? { ...item, completed } : item) });
    try {
      const todo = await api.setTodoCompleted(id, completed);
      set((state) => ({ todos: state.todos.map((item) => item.id === id ? todo : item), error: null }));
    } catch (error) { set({ todos: previous, error: message(error) }); }
  },
  removeTodo: async (id) => {
    try {
      await api.deleteTodo(id);
      set((state) => ({ todos: state.todos.filter((item) => item.id !== id), error: null }));
    } catch (error) { set({ error: message(error) }); }
  },
  reorder: async (orderedIds) => {
    const byId = new Map(get().todos.map((todo) => [todo.id, todo]));
    const ordered = orderedIds.map((id, index) => ({ ...byId.get(id)!, sortOrder: index * 1024 }));
    const remaining = get().todos.filter((todo) => !orderedIds.includes(todo.id));
    set({ todos: [...ordered, ...remaining] });
    try { await api.reorderTodos(orderedIds); }
    catch (error) { set({ error: message(error) }); await get().initialize(); }
  },
  saveLayout: async (layout) => {
    set({ layout });
    try { await api.saveWidgetLayout(layout); }
    catch (error) { set({ error: message(error) }); }
  },
  saveSettings: async (settings) => {
    try {
      const saved = await api.updateSettings(settings);
      set({ settings: saved, error: null });
    } catch (error) { set({ error: message(error) }); }
  },
  setMode: async (mode, notifyBackend = true) => {
    set({ interactionMode: mode });
    if (notifyBackend) {
      try { await api.setInteractionMode(mode, get().layout); }
      catch (error) { set({ error: message(error) }); }
    }
  },
  clearError: () => set({ error: null }),
}));
