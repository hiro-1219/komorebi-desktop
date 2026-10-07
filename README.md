# Todo Clear

Windowsのデスクトップに半透明のToDoカードを表示する、ローカルファーストのTauriアプリです。

## 開発

必要環境: Node.js 22.12以降（LTS推奨）、Rust 1.90以降、Windows WebView2。

```powershell
npm install
npm run tauri dev
```

`Ctrl+Alt+W`またはトレイメニューで、表示モードと編集モードを切り替えます。通常時はカードがデスクトップアイコンの背面に固定され、マウス操作を妨げません。

## 検証

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

データはOSのアプリデータディレクトリにある`todo-clear.sqlite3`へ保存されます。
