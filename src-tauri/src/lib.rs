use chrono::{NaiveDateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize,
    State, WebviewWindow,
};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

type CommandResult<T> = Result<T, String>;

struct Database(Mutex<Connection>);
struct RuntimeState {
    editing: AtomicBool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Todo {
    id: String,
    title: String,
    completed: bool,
    due_at: Option<String>,
    priority: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodoInput {
    title: String,
    due_at: Option<String>,
    priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WidgetLayout {
    id: String,
    kind: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    settings: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    auto_start: bool,
    opacity: f64,
    #[serde(default = "default_shadow_strength")]
    shadow_strength: f64,
    theme: String,
}

fn default_shadow_strength() -> f64 {
    0.0
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_start: false,
            opacity: 0.7,
            shadow_strength: default_shadow_strength(),
            theme: "system".into(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BoardState {
    widgets: Vec<WidgetLayout>,
    settings: AppSettings,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopPalette {
    surface: String,
    wallpaper_luminance: f64,
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
fn db_err(error: impl std::fmt::Display) -> String {
    format!("データベースエラー: {error}")
}

fn validate_input(input: &TodoInput) -> CommandResult<(String, Option<String>, String)> {
    let title = input.title.trim().to_string();
    if title.is_empty() {
        return Err("タイトルを入力してください".into());
    }
    if title.chars().count() > 200 {
        return Err("タイトルは200文字以内で入力してください".into());
    }
    if !["none", "low", "medium", "high"].contains(&input.priority.as_str()) {
        return Err("優先度が正しくありません".into());
    }
    if let Some(date_time) = &input.due_at {
        NaiveDateTime::parse_from_str(date_time, "%Y-%m-%dT%H:%M")
            .map_err(|_| "期限の日時が正しくありません".to_string())?;
    }
    Ok((title, input.due_at.clone(), input.priority.clone()))
}

fn todo_from_row(row: &Row<'_>) -> rusqlite::Result<Todo> {
    Ok(Todo {
        id: row.get(0)?,
        title: row.get(1)?,
        completed: row.get::<_, i64>(2)? != 0,
        due_at: row.get(3)?,
        priority: row.get(4)?,
        sort_order: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        completed_at: row.get(8)?,
    })
}

fn database_path(app: &AppHandle) -> CommandResult<PathBuf> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| format!("データ保存先を作成できません: {e}"))?;
    Ok(dir.join("todo-clear.sqlite3"))
}

fn open_database(app: &AppHandle) -> CommandResult<Connection> {
    let connection = Connection::open(database_path(app)?).map_err(db_err)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(db_err)?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(db_err)?;
    connection.execute_batch(
        "BEGIN;
         CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS todos (
           id TEXT PRIMARY KEY, title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 200),
           completed INTEGER NOT NULL DEFAULT 0, due_date TEXT, priority TEXT NOT NULL DEFAULT 'none',
           sort_order INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, completed_at TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_todos_order ON todos(sort_order);
         CREATE TABLE IF NOT EXISTS widget_layouts (
           id TEXT PRIMARY KEY, kind TEXT NOT NULL, x REAL NOT NULL, y REAL NOT NULL,
           width REAL NOT NULL, height REAL NOT NULL, settings_json TEXT NOT NULL DEFAULT '{}'
         );
         CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL);
         INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (1, datetime('now'));
         INSERT OR IGNORE INTO widget_layouts(id, kind, x, y, width, height, settings_json)
           VALUES ('todo-main', 'todo-list', 40, 72, 380, 590, '{}');
         COMMIT;"
    ).map_err(db_err)?;
    let has_due_at: bool = connection
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('todos') WHERE name='due_at'",
            [],
            |row| row.get(0),
        )
        .map_err(db_err)?;
    if !has_due_at {
        connection
            .execute_batch(
                "BEGIN;
                 ALTER TABLE todos ADD COLUMN due_at TEXT;
                 UPDATE todos SET due_at = due_date || 'T23:59' WHERE due_date IS NOT NULL;
                 INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (2, datetime('now'));
                 COMMIT;",
            )
            .map_err(db_err)?;
    }
    Ok(connection)
}

#[tauri::command]
fn list_todos(db: State<Database>) -> CommandResult<Vec<Todo>> {
    let connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    let mut statement = connection.prepare(
        "SELECT id,title,completed,due_at,priority,sort_order,created_at,updated_at,completed_at FROM todos ORDER BY sort_order"
    ).map_err(db_err)?;
    let todos = statement
        .query_map([], todo_from_row)
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;
    Ok(todos)
}

#[tauri::command]
fn create_todo(input: TodoInput, db: State<Database>) -> CommandResult<Todo> {
    let (title, due_at, priority) = validate_input(&input)?;
    let connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    let sort_order = connection
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1024) + 1024 FROM todos",
            [],
            |r| r.get(0),
        )
        .map_err(db_err)?;
    let todo = Todo {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        completed: false,
        due_at,
        priority,
        sort_order,
        created_at: now(),
        updated_at: now(),
        completed_at: None,
    };
    connection
        .execute(
            "INSERT INTO todos (id,title,completed,due_at,priority,sort_order,created_at,updated_at,completed_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                todo.id,
                todo.title,
                todo.completed as i64,
                todo.due_at,
                todo.priority,
                todo.sort_order,
                todo.created_at,
                todo.updated_at,
                todo.completed_at
            ],
        )
        .map_err(db_err)?;
    Ok(todo)
}

#[tauri::command]
fn update_todo(id: String, input: TodoInput, db: State<Database>) -> CommandResult<Todo> {
    let (title, due_at, priority) = validate_input(&input)?;
    let connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    let changed = connection
        .execute(
            "UPDATE todos SET title=?1,due_at=?2,priority=?3,updated_at=?4 WHERE id=?5",
            params![title, due_at, priority, now(), id],
        )
        .map_err(db_err)?;
    if changed == 0 {
        return Err("タスクが見つかりません".into());
    }
    connection.query_row("SELECT id,title,completed,due_at,priority,sort_order,created_at,updated_at,completed_at FROM todos WHERE id=?1", [id], todo_from_row).map_err(db_err)
}

#[tauri::command]
fn set_todo_completed(id: String, completed: bool, db: State<Database>) -> CommandResult<Todo> {
    let connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    let completed_at = completed.then(now);
    let changed = connection
        .execute(
            "UPDATE todos SET completed=?1,completed_at=?2,updated_at=?3 WHERE id=?4",
            params![completed as i64, completed_at, now(), id],
        )
        .map_err(db_err)?;
    if changed == 0 {
        return Err("タスクが見つかりません".into());
    }
    connection.query_row("SELECT id,title,completed,due_at,priority,sort_order,created_at,updated_at,completed_at FROM todos WHERE id=?1", [id], todo_from_row).map_err(db_err)
}

#[tauri::command]
fn delete_todo(id: String, db: State<Database>) -> CommandResult<()> {
    let connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    if connection
        .execute("DELETE FROM todos WHERE id=?1", [id])
        .map_err(db_err)?
        == 0
    {
        return Err("タスクが見つかりません".into());
    }
    Ok(())
}

#[tauri::command]
fn reorder_todos(ordered_ids: Vec<String>, db: State<Database>) -> CommandResult<()> {
    let mut connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    let transaction = connection.transaction().map_err(db_err)?;
    for (index, id) in ordered_ids.iter().enumerate() {
        if transaction
            .execute(
                "UPDATE todos SET sort_order=?1,updated_at=?2 WHERE id=?3",
                params![index as i64 * 1024, now(), id],
            )
            .map_err(db_err)?
            == 0
        {
            return Err(format!("タスクが見つかりません: {id}"));
        }
    }
    transaction.commit().map_err(db_err)
}

fn read_settings(connection: &Connection) -> CommandResult<AppSettings> {
    let value: Option<String> = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key='general'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_err)?;
    value
        .map(|json| serde_json::from_str(&json).map_err(|e| format!("設定を読み込めません: {e}")))
        .unwrap_or_else(|| Ok(AppSettings::default()))
}

#[tauri::command]
fn get_board_state(db: State<Database>) -> CommandResult<BoardState> {
    let connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    let mut statement = connection
        .prepare("SELECT id,kind,x,y,width,height,settings_json FROM widget_layouts ORDER BY id")
        .map_err(db_err)?;
    let widgets = statement
        .query_map([], |row| {
            Ok(WidgetLayout {
                id: row.get(0)?,
                kind: row.get(1)?,
                x: row.get(2)?,
                y: row.get(3)?,
                width: row.get(4)?,
                height: row.get(5)?,
                settings: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
            })
        })
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;
    Ok(BoardState {
        widgets,
        settings: read_settings(&connection)?,
    })
}

#[tauri::command]
fn save_widget_layout(layout: WidgetLayout, db: State<Database>) -> CommandResult<()> {
    if layout.kind != "todo-list"
        || layout.width < 320.0
        || layout.height < 380.0
        || ![layout.x, layout.y, layout.width, layout.height]
            .iter()
            .all(|v| v.is_finite())
    {
        return Err("ウィジェット配置が正しくありません".into());
    }
    let connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    connection.execute("INSERT INTO widget_layouts VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET kind=excluded.kind,x=excluded.x,y=excluded.y,width=excluded.width,height=excluded.height,settings_json=excluded.settings_json", params![layout.id,layout.kind,layout.x,layout.y,layout.width,layout.height,layout.settings.to_string()]).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
fn update_settings(
    settings: AppSettings,
    app: AppHandle,
    db: State<Database>,
) -> CommandResult<AppSettings> {
    if !(0.3..=0.95).contains(&settings.opacity)
        || !(0.0..=1.0).contains(&settings.shadow_strength)
        || !["system", "light", "dark"].contains(&settings.theme.as_str())
    {
        return Err("設定値が正しくありません".into());
    }
    if settings.auto_start {
        app.autolaunch()
            .enable()
            .map_err(|e| format!("自動起動を有効にできません: {e}"))?;
    } else {
        app.autolaunch()
            .disable()
            .map_err(|e| format!("自動起動を無効にできません: {e}"))?;
    }
    let json = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    let connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    connection.execute("INSERT INTO app_settings(key,value_json) VALUES('general',?1) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json", [json]).map_err(db_err)?;
    Ok(settings)
}

#[tauri::command]
fn get_desktop_palette() -> DesktopPalette {
    #[cfg(target_os = "windows")]
    {
        let wallpaper = std::env::var_os("APPDATA").map(PathBuf::from).map(|path| {
            path.join("Microsoft")
                .join("Windows")
                .join("Themes")
                .join("TranscodedWallpaper")
        });
        if let Some(path) = wallpaper {
            if let Ok(bytes) = fs::read(path) {
                if let Ok(image) = image::load_from_memory(&bytes) {
                    let pixels = image.thumbnail(64, 64).to_rgb8();
                    let count = pixels.width() as f64 * pixels.height() as f64;
                    if count > 0.0 {
                        let luminance = pixels
                            .pixels()
                            .map(|pixel| {
                                (0.2126 * pixel[0] as f64
                                    + 0.7152 * pixel[1] as f64
                                    + 0.0722 * pixel[2] as f64)
                                    / 255.0
                            })
                            .sum::<f64>()
                            / count;
                        return DesktopPalette {
                            // Choose the opposite surface tone to keep the card edge and text
                            // readable without hiding the wallpaper completely.
                            surface: if luminance >= 0.48 { "dark" } else { "light" }.into(),
                            wallpaper_luminance: luminance,
                        };
                    }
                }
            }
        }
    }
    DesktopPalette {
        surface: "dark".into(),
        wallpaper_luminance: 0.5,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum InteractionMode {
    Locked,
    Editing,
}

#[cfg(target_os = "windows")]
fn set_underlay(window: &WebviewWindow, enabled: bool) -> CommandResult<()> {
    use windows::{
        core::{s, BOOL},
        Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::WindowsAndMessaging::{
                EnumWindows, FindWindowA, FindWindowExA, SendMessageTimeoutA, SetParent,
                SMTO_NORMAL,
            },
        },
    };

    unsafe extern "system" fn find_worker(window: HWND, output: LPARAM) -> BOOL {
        unsafe {
            let shell_view =
                FindWindowExA(Some(window), None, s!("SHELLDLL_DefView"), None).unwrap_or_default();
            if shell_view.is_invalid() {
                return true.into();
            }
            let worker = FindWindowExA(None, Some(window), s!("WorkerW"), None).unwrap_or_default();
            if !worker.is_invalid() {
                *(output.0 as *mut HWND) = worker;
            }
            true.into()
        }
    }

    let hwnd = window
        .hwnd()
        .map_err(|e| format!("ウィンドウハンドルを取得できません: {e}"))?;
    unsafe {
        if !enabled {
            SetParent(hwnd, None).map_err(|e| format!("デスクトップから切り離せません: {e}"))?;
            return Ok(());
        }
        let progman = FindWindowA(s!("Progman"), None)
            .map_err(|e| format!("Windowsデスクトップが見つかりません: {e}"))?;
        let _ = SendMessageTimeoutA(
            progman,
            0x052C,
            WPARAM(0x0000000D),
            LPARAM(1),
            SMTO_NORMAL,
            1000,
            None,
        );
        let mut worker = HWND::default();
        EnumWindows(Some(find_worker), LPARAM(&mut worker as *mut HWND as isize))
            .map_err(|e| format!("デスクトップを列挙できません: {e}"))?;
        if worker.is_invalid() {
            worker = FindWindowExA(Some(progman), None, s!("WorkerW"), None)
                .map_err(|e| format!("WorkerWが見つかりません: {e}"))?;
        }
        if worker.is_invalid() {
            return Err("デスクトップのWorkerWが見つかりません".into());
        }
        SetParent(hwnd, Some(worker)).map_err(|e| format!("デスクトップへ配置できません: {e}"))?;
    }
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn set_underlay(_window: &WebviewWindow, _enabled: bool) -> CommandResult<()> {
    Ok(())
}

fn stored_widget_layout(app: &AppHandle) -> CommandResult<WidgetLayout> {
    let db = app.state::<Database>();
    let connection =
        db.0.lock()
            .map_err(|_| "データベースのロックに失敗しました".to_string())?;
    connection
        .query_row(
            "SELECT id,kind,x,y,width,height,settings_json FROM widget_layouts WHERE id='todo-main'",
            [],
            |row| {
                Ok(WidgetLayout {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    x: row.get(2)?,
                    y: row.get(3)?,
                    width: row.get(4)?,
                    height: row.get(5)?,
                    settings: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                })
            },
        )
        .map_err(db_err)
}

fn size_to_widget(window: &WebviewWindow, layout: &WidgetLayout) -> CommandResult<()> {
    let monitor = window
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("主モニターが見つかりません")?;
    let scale = monitor.scale_factor();
    let monitor_x = monitor.position().x as f64 / scale;
    let monitor_y = monitor.position().y as f64 / scale;
    window
        .set_position(LogicalPosition::new(
            monitor_x + layout.x,
            monitor_y + layout.y,
        ))
        .map_err(|e| e.to_string())?;
    window
        .set_size(LogicalSize::new(layout.width, layout.height))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn apply_interaction_mode(
    app: &AppHandle,
    mode: InteractionMode,
    requested_layout: Option<&WidgetLayout>,
) -> CommandResult<()> {
    let window = app
        .get_webview_window("main")
        .ok_or("メインウィンドウが見つかりません")?;
    let editing = matches!(mode, InteractionMode::Editing);
    if editing {
        set_underlay(&window, false)?;
        size_to_primary_monitor(&window)?;
        window
            .set_ignore_cursor_events(false)
            .map_err(|e| e.to_string())?;
        window.set_always_on_top(true).map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        set_underlay(&window, false)?;
        window.set_always_on_top(false).map_err(|e| e.to_string())?;
        window
            .set_ignore_cursor_events(false)
            .map_err(|e| e.to_string())?;
        let stored;
        let layout = if let Some(layout) = requested_layout {
            layout
        } else {
            stored = stored_widget_layout(app)?;
            &stored
        };
        size_to_widget(&window, layout)?;
        window.show().map_err(|e| e.to_string())?;
    }
    app.state::<RuntimeState>()
        .editing
        .store(editing, Ordering::SeqCst);
    app.emit(
        "interaction-mode-changed",
        if editing { "editing" } else { "locked" },
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn set_interaction_mode(
    mode: InteractionMode,
    layout: Option<WidgetLayout>,
    app: AppHandle,
) -> CommandResult<()> {
    apply_interaction_mode(&app, mode, layout.as_ref())
}

fn toggle_interaction_mode(app: &AppHandle) {
    let editing = app.state::<RuntimeState>().editing.load(Ordering::SeqCst);
    let _ = apply_interaction_mode(
        app,
        if editing {
            InteractionMode::Locked
        } else {
            InteractionMode::Editing
        },
        None,
    );
}

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let toggle = MenuItem::with_id(app, "toggle", "編集モードを切り替え", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "設定…", true, None::<&str>)?;
    let autostart = MenuItem::with_id(app, "autostart", "自動起動設定…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &settings, &autostart, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Todo Clear")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => toggle_interaction_mode(app),
            "settings" | "autostart" => {
                let _ = apply_interaction_mode(app, InteractionMode::Editing, None);
                let _ = app.emit("open-settings", ());
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn size_to_primary_monitor(window: &WebviewWindow) -> CommandResult<()> {
    if let Some(monitor) = window.primary_monitor().map_err(|e| e.to_string())? {
        window
            .set_position(PhysicalPosition::new(
                monitor.position().x,
                monitor.position().y,
            ))
            .map_err(|e| e.to_string())?;
        window
            .set_size(PhysicalSize::new(
                monitor.size().width,
                monitor.size().height,
            ))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyW);
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = apply_interaction_mode(app, InteractionMode::Editing, None);
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Todo Clear")
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_interaction_mode(app);
                    }
                })
                .build(),
        )
        .setup(move |app| {
            app.manage(Database(Mutex::new(
                open_database(app.handle()).map_err(std::io::Error::other)?,
            )));
            app.manage(RuntimeState {
                editing: AtomicBool::new(false),
            });
            setup_tray(app.handle())?;
            if let Err(error) = app.global_shortcut().register(shortcut) {
                eprintln!("Global shortcut unavailable: {error}");
            }
            apply_interaction_mode(app.handle(), InteractionMode::Locked, None)
                .map_err(std::io::Error::other)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_todos,
            create_todo,
            update_todo,
            set_todo_completed,
            delete_todo,
            reorder_todos,
            get_board_state,
            save_widget_layout,
            update_settings,
            get_desktop_palette,
            set_interaction_mode
        ])
        .run(tauri::generate_context!())
        .expect("Todo Clear failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_todo_input() {
        assert!(validate_input(&TodoInput {
            title: " ".into(),
            due_at: None,
            priority: "none".into()
        })
        .is_err());
        assert!(validate_input(&TodoInput {
            title: "Task".into(),
            due_at: Some("2026-10-07T18:30".into()),
            priority: "high".into()
        })
        .is_ok());
        assert!(validate_input(&TodoInput {
            title: "Task".into(),
            due_at: Some("bad".into()),
            priority: "high".into()
        })
        .is_err());
    }
}
