import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import { dueState, formatDueAt, validateTitle, visibleTodos } from "./domain";
import { useAppStore } from "./store";
import type { AppSettings, InteractionMode, Priority, Todo, TodoFilter, TodoInput, TodoSort, WidgetLayout } from "./types";

const priorities: Array<{ value: Priority; label: string }> = [
  { value: "none", label: "優先度なし" }, { value: "low", label: "低" },
  { value: "medium", label: "中" }, { value: "high", label: "高" },
];

function TodoForm({ initial, onSave, onCancel }: {
  initial?: Todo; onSave: (input: TodoInput) => Promise<void>; onCancel?: () => void;
}) {
  const [title, setTitle] = useState(initial?.title ?? "");
  const [dueAt, setDueAt] = useState(initial?.dueAt ?? "");
  const [priority, setPriority] = useState<Priority>(initial?.priority ?? "none");
  const [validation, setValidation] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    const problem = validateTitle(title);
    if (problem) { setValidation(problem); return; }
    setSaving(true);
    try {
      await onSave({ title: title.trim(), dueAt: dueAt || null, priority });
      if (!initial) { setTitle(""); setDueAt(""); setPriority("none"); }
      setValidation(null);
    } finally { setSaving(false); }
  };

  return <form className={initial ? "edit-form" : "add-form"} onSubmit={submit}>
    <input aria-label="タスク名" className="title-input" value={title} onChange={(event) => setTitle(event.target.value)}
      placeholder="新しいタスク…" maxLength={200} autoFocus={Boolean(initial)} />
    <div className="form-details">
      <input aria-label="期限日時" type="datetime-local" value={dueAt} onChange={(event) => setDueAt(event.target.value)} />
      <select aria-label="優先度" value={priority} onChange={(event) => setPriority(event.target.value as Priority)}>
        {priorities.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
      </select>
      <button className="primary" disabled={saving} type="submit">{initial ? "保存" : "追加"}</button>
      {onCancel && <button type="button" onClick={onCancel}>取消</button>}
    </div>
    {validation && <p className="field-error">{validation}</p>}
  </form>;
}

function TodoRow({ todo, editing, editable, onEdit, draggable, onDragStart, onDrop }: {
  todo: Todo; editing: boolean; editable: boolean; onEdit: (id: string | null) => void; draggable: boolean;
  onDragStart: (id: string) => void; onDrop: (id: string) => void;
}) {
  const { updateTodo, toggleTodo, removeTodo } = useAppStore();
  if (editing) return <li className="todo-row editing"><TodoForm initial={todo}
    onSave={async (input) => { await updateTodo(todo.id, input); onEdit(null); }} onCancel={() => onEdit(null)} /></li>;
  const state = dueState(todo.dueAt);
  return <li className={`todo-row ${todo.completed ? "completed" : ""}`} draggable={draggable}
    onDragStart={() => onDragStart(todo.id)} onDragOver={(event) => event.preventDefault()} onDrop={() => onDrop(todo.id)}>
    <span className={`priority-dot ${todo.priority}`} aria-label={`優先度 ${todo.priority}`} />
    <input className="check" type="checkbox" checked={todo.completed}
      onChange={(event) => void toggleTodo(todo.id, event.target.checked)} aria-label={`${todo.title}を完了`} />
    <div className="todo-copy">
      <span className="todo-title">{todo.title}</span>
      {todo.dueAt && <time className={`due ${state}`} dateTime={todo.dueAt}>
        {state === "overdue" ? `期限超過 · ${formatDueAt(todo.dueAt)}` : state === "today" ? `今日 ${formatDueAt(todo.dueAt).split(" ").at(-1)}` : formatDueAt(todo.dueAt)}
      </time>}
    </div>
    {editable && <div className="row-actions">
      <button aria-label="編集" onClick={() => onEdit(todo.id)}>✎</button>
      <button aria-label="削除" onClick={() => { if (confirm(`「${todo.title}」を削除しますか？`)) void removeTodo(todo.id); }}>×</button>
    </div>}
  </li>;
}

function TodoWidget({ editable }: { editable: boolean }) {
  const { todos, addTodo, reorder } = useAppStore();
  const [filter, setFilter] = useState<TodoFilter>("active");
  const [sort, setSort] = useState<TodoSort>("manual");
  const [editingId, setEditingId] = useState<string | null>(null);
  const dragged = useRef<string | null>(null);
  const shown = useMemo(() => visibleTodos(todos, filter, sort), [todos, filter, sort]);
  const completedCount = todos.filter((todo) => todo.completed).length;

  const drop = (targetId: string) => {
    if (!dragged.current || dragged.current === targetId || sort !== "manual") return;
    const ids = shown.map((todo) => todo.id);
    const from = ids.indexOf(dragged.current); const to = ids.indexOf(targetId);
    ids.splice(to, 0, ids.splice(from, 1)[0]);
    const visibleIds = new Set(ids);
    const reordered = [...ids];
    const allIds = [...todos].sort((a, b) => a.sortOrder - b.sortOrder)
      .map((todo) => visibleIds.has(todo.id) ? reordered.shift()! : todo.id);
    void reorder(allIds); dragged.current = null;
  };

  return <div className="widget-content">
    {editable && <TodoForm onSave={addTodo} />}
    <div className="toolbar">
      <div className="segmented" aria-label="表示フィルター">
        {(["active", "all", "completed"] as TodoFilter[]).map((value) =>
          <button key={value} className={filter === value ? "selected" : ""} onClick={() => setFilter(value)}>
            {value === "active" ? "未完了" : value === "all" ? "すべて" : `完了 ${completedCount}`}
          </button>)}
      </div>
      <select aria-label="並べ替え" value={sort} onChange={(event) => setSort(event.target.value as TodoSort)}>
        <option value="manual">手動順</option><option value="due">期限順</option><option value="priority">優先度順</option>
      </select>
    </div>
    <ul className="todo-list">
      {shown.map((todo) => <TodoRow key={todo.id} todo={todo} editing={editable && editingId === todo.id} editable={editable}
        onEdit={setEditingId} draggable={sort === "manual"} onDragStart={(id) => { dragged.current = id; }} onDrop={drop} />)}
    </ul>
    {!shown.length && <div className="empty"><span>✓</span><p>{filter === "completed" ? "完了したタスクはありません" : "きれいな一日です"}</p></div>}
  </div>;
}

function SettingsPanel({ settings, onClose }: { settings: AppSettings; onClose: () => void }) {
  const { saveSettings } = useAppStore();
  const [draft, setDraft] = useState(settings);
  return <div className="settings-backdrop" role="dialog" aria-modal="true" aria-label="設定">
    <section className="settings-panel"><header><div><span className="eyebrow">PREFERENCES</span><h2>設定</h2></div><button onClick={onClose}>×</button></header>
      <label className="setting-row"><span><strong>ログイン時に起動</strong><small>Windowsへのログイン後、自動で表示します</small></span>
        <input type="checkbox" checked={draft.autoStart} onChange={(event) => setDraft({ ...draft, autoStart: event.target.checked })} /></label>
      <label className="setting-stack"><span><strong>カードの透明度</strong><output>{Math.round(draft.opacity * 100)}%</output></span>
        <input type="range" min="0.3" max="0.95" step="0.05" value={draft.opacity}
          onChange={(event) => setDraft({ ...draft, opacity: Number(event.target.value) })} /></label>
      <label className="setting-stack"><span><strong>影の強さ</strong><output>{Math.round(draft.shadowStrength * 100)}%</output></span>
        <input type="range" min="0" max="1" step="0.05" value={draft.shadowStrength}
          onChange={(event) => setDraft({ ...draft, shadowStrength: Number(event.target.value) })} /></label>
      <label className="setting-stack"><strong>テーマ</strong><select value={draft.theme}
        onChange={(event) => setDraft({ ...draft, theme: event.target.value as AppSettings["theme"] })}>
        <option value="system">システム設定</option><option value="light">ライト</option><option value="dark">ダーク</option>
      </select></label>
      <footer><button onClick={onClose}>キャンセル</button><button className="primary" onClick={() => void saveSettings(draft).then(onClose)}>保存</button></footer>
    </section>
  </div>;
}

function App() {
  const { initialize, loading, layout, saveLayout, settings, interactionMode, setMode, error, clearError } = useAppStore();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [desktopSurface, setDesktopSurface] = useState<"light" | "dark">("dark");
  const gesture = useRef<{ type: "move" | "resize"; startX: number; startY: number; layout: WidgetLayout } | null>(null);

  useEffect(() => { void initialize(); }, [initialize]);
  useEffect(() => {
    const unlisteners = [
      listen<InteractionMode>("interaction-mode-changed", (event) => void setMode(event.payload, false)),
      listen("open-settings", () => { void setMode("editing", false); setSettingsOpen(true); }),
    ];
    return () => { void Promise.all(unlisteners).then((items) => items.forEach((unlisten) => unlisten())); };
  }, [setMode]);
  useEffect(() => {
    document.documentElement.dataset.theme = settings.theme;
    document.documentElement.dataset.surface = settings.theme === "system" ? desktopSurface : settings.theme;
  }, [settings.theme, desktopSurface]);
  useEffect(() => {
    const refresh = () => { void api.getDesktopPalette().then((palette) => setDesktopSurface(palette.surface)).catch(() => undefined); };
    refresh();
    const timer = window.setInterval(refresh, 60_000);
    return () => window.clearInterval(timer);
  }, []);
  useEffect(() => {
    if (interactionMode === "locked") setSettingsOpen(false);
  }, [interactionMode]);
  useEffect(() => {
    if (!layout || interactionMode !== "editing" || innerWidth <= 0 || innerHeight <= 0) return;
    const next = {
      ...layout,
      width: Math.min(layout.width, Math.max(320, innerWidth - 16)),
      height: Math.min(layout.height, Math.max(380, innerHeight - 16)),
      x: Math.max(8, Math.min(innerWidth - Math.min(layout.width, innerWidth - 16) - 8, layout.x)),
      y: Math.max(8, Math.min(innerHeight - Math.min(layout.height, innerHeight - 16) - 8, layout.y)),
    };
    if (next.x !== layout.x || next.y !== layout.y || next.width !== layout.width || next.height !== layout.height) void saveLayout(next);
  }, [interactionMode, layout, saveLayout]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => { if (event.key === "Escape" && interactionMode === "editing") { setSettingsOpen(false); void setMode("locked"); } };
    window.addEventListener("keydown", key); return () => window.removeEventListener("keydown", key);
  }, [interactionMode, setMode]);

  const move = useCallback((event: PointerEvent) => {
    if (!gesture.current) return;
    const { type, startX, startY, layout: initial } = gesture.current;
    const dx = event.clientX - startX, dy = event.clientY - startY;
    const next = type === "move"
      ? { ...initial, x: Math.max(8, Math.min(innerWidth - initial.width - 8, initial.x + dx)), y: Math.max(8, Math.min(innerHeight - initial.height - 8, initial.y + dy)) }
      : { ...initial, width: Math.max(320, Math.min(innerWidth - initial.x - 8, initial.width + dx)), height: Math.max(380, Math.min(innerHeight - initial.y - 8, initial.height + dy)) };
    useAppStore.setState({ layout: next });
  }, []);
  useEffect(() => {
    const up = () => { if (gesture.current && useAppStore.getState().layout) void saveLayout(useAppStore.getState().layout!); gesture.current = null; };
    window.addEventListener("pointermove", move); window.addEventListener("pointerup", up);
    return () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); };
  }, [move, saveLayout]);

  const beginGesture = (type: "move" | "resize", event: React.PointerEvent) => {
    if (!layout || interactionMode !== "editing") return;
    gesture.current = { type, startX: event.clientX, startY: event.clientY, layout };
    event.preventDefault();
  };
  const nudgeWidget = (event: React.KeyboardEvent) => {
    if (!layout || !["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
    event.preventDefault();
    const distance = event.shiftKey ? 10 : 1;
    const dx = event.key === "ArrowLeft" ? -distance : event.key === "ArrowRight" ? distance : 0;
    const dy = event.key === "ArrowUp" ? -distance : event.key === "ArrowDown" ? distance : 0;
    const next = {
      ...layout,
      x: Math.max(8, Math.min(innerWidth - layout.width - 8, layout.x + dx)),
      y: Math.max(8, Math.min(innerHeight - layout.height - 8, layout.y + dy)),
    };
    void saveLayout(next);
  };
  if (loading || !layout) return <div className="boot">Todo Clear</div>;
  return <main className={`desktop-board ${interactionMode}`}>
    {interactionMode === "editing" && <div className="edit-bar"><span><i /> 編集モード</span>
      <div><button onClick={() => setSettingsOpen(true)}>設定</button><button className="primary" onClick={() => void setMode("locked")}>完了</button></div></div>}
    <section className="widget" style={{
      left: layout.x, top: layout.y, width: layout.width, height: layout.height,
      "--card-opacity": settings.opacity,
      "--shadow-alpha": settings.shadowStrength * 0.45,
      "--shadow-y": `${4 + settings.shadowStrength * 24}px`,
      "--shadow-blur": `${12 + settings.shadowStrength * 68}px`,
    } as React.CSSProperties}>
      {interactionMode === "editing" && <div className="drag-handle" role="button" tabIndex={0}
        onPointerDown={(event) => beginGesture("move", event)} onKeyDown={nudgeWidget}
        aria-label="ドラッグまたは矢印キーでカードを移動"><span>⠿</span> ドラッグして移動</div>}
      <TodoWidget editable={interactionMode === "editing"} />
      {interactionMode === "editing" && <div className="resize-handle" onPointerDown={(event) => beginGesture("resize", event)} aria-label="カードのサイズを変更" />}
    </section>
    {settingsOpen && <SettingsPanel settings={settings} onClose={() => setSettingsOpen(false)} />}
    {error && <div className="toast" role="alert"><span>{error}</span><button onClick={clearError}>×</button></div>}
  </main>;
}

export default App;
