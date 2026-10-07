# komorebi-desktop

Windowsデスクトップに、半透明のToDoカードをそっと置いておける常駐アプリです。壁紙になじみながら、タスクの追加・完了・期限管理・並べ替えを行えます。

プロダクト名は **komorebi-desktop**。将来的にカレンダーなどのWidgetを同じデスクトップボードへ追加できる構成にしています。

## Features

- 半透明・壁紙適応型のデスクトップカード
- タスクの追加、編集、完了、削除
- 期限の日付・時刻、優先度、期限超過表示
- 未完了／すべて／完了済みのフィルター
- 手動順、期限順、優先度順の並べ替え
- 通常表示でも完了チェックと手動並べ替えが可能
- 編集モードでカードの移動・サイズ変更
- ライト／ダーク／壁紙適応テーマ
- カード透明度・影の強さの設定
- SQLiteによるローカル保存（オフライン動作）
- システムトレイ、単一起動、自動起動、グローバルショートカット

## 操作方法

`Ctrl+Alt+W`またはシステムトレイから編集モードを切り替えます。

- 通常表示：カード内のチェック、フィルター、手動並べ替えを操作できます
- 編集モード：新規タスクの追加、編集、削除、カードの移動・サイズ変更ができます
- 編集モードのカード上部をドラッグ：カードを移動
- カード右下をドラッグ：カードをリサイズ
- `Escape`：編集モードを終了

## Tech stack

- Tauri 2 / Rust
- React / TypeScript / Vite
- SQLite（rusqlite）
- Windows WebView2

## 日常利用向けインストール

リリース版NSISインストーラーを生成します。

```powershell
npm install
npm run package:nsis
```

生成された`src-tauri/target/release/bundle/nsis/`内のセットアップexeを実行してください。現在のユーザー単位でインストールされるため、通常は管理者権限を必要としません。Windows WebView2が未導入の場合は、インストーラーがMicrosoftのBootstrapperを取得して導入します。

インストール後はスタートメニューから`komorebi-desktop`を起動できます。アプリはトレイに常駐し、設定からログイン時の自動起動を有効化できます。更新時は新しいセットアップexeを実行してください。

初版はコード署名を行っていないため、Windows SmartScreenの警告が表示される場合があります。タスクやカード配置などのデータはアンインストール後も削除せず、再インストール時に引き継ぎます。

## Development

必要環境：

- Windows 10/11
- Node.js 22.12以降（LTS推奨）
- Rust 1.90以降
- Windows WebView2

依存関係をインストールして開発起動します。

```powershell
npm install
npm run tauri dev
```

## Validation

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run test:all
npm run package:debug
```

デバッグ実行形式は`src-tauri/target/debug/todo-clear.exe`に生成されます。正式インストーラーは`src-tauri/target/release/bundle/nsis/`に生成されます。

## Data

タスク、カード配置、テーマ、透明度などの設定は、OSのアプリデータディレクトリにあるSQLiteデータベースへ保存されます。内部識別子を維持しているため、旧Todo Clear版からのデータも引き継がれます。ネットワーク接続やアカウントは必要ありません（WebView2の初回導入を除く）。

## Scope

現在はWindowsとToDo Widgetを中心に実装しています。クラウド同期、通知、繰り返しタスク、タグ、サブタスク、macOS対応は今後の拡張候補です。
