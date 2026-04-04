<p align="center">
  <img src="icon.svg" width="128" height="128" alt="focus_other_display icon">
</p>

# focus_other_display

macOS のデュアルディスプレイ環境で、反対側のディスプレイの最前面ウィンドウにフォーカスを切り替えるツール。

## 必要な環境

- macOS
- Rust (nightly) — edition 2024 を使用
- デュア���ディスプレイ構成

### macOS 権限

- **アクセシビリティ** (システム設定 > プライバシーとセキュリティ > アクセシビリティ) — ウィンドウの操作に必要
- **画面収録** (同 > 画面収録とシステムオーディオ録音) �� ウィンドウタイトルの取得に必要（なくても動作する）

## ビルド・実行

```sh
cargo build --release
./target/release/focus_other_display
```

## 動作

1. 現在フォーカス中のウィンドウがどのディスプレイにあるか判定
2. 反対側のディスプレイで最前面にあるウィンド��を特定
3. マウスカーソルをそのウィンドウの中央に移動
4. `AXRaise` でウィンドウを前面化し、キーボー���フォーカスを移動

```
OK: 右 [WezTerm] → 左 [Google Chrome - GitHub]
```

## キーボードショートカットへの登録例

[Hammerspoon](https://www.hammerspoon.org/) で `Ctrl+Space` に割り当てる場合:

```lua
hs.hotkey.bind({"ctrl"}, "space", function()
  hs.task.new("/path/to/focus_other_display", nil):start()
end)
```

## プロジェクト構成

```
src/
  main.rs           -- エントリポイント、メ��ンロジック
  appkit.rs          -- フロントアプリ取得 (NSWorkspace)
  display.rs         -- ディスプレイ情報取得 (NSScreen)
  window.rs          -- ウィン���ウ一括取得 (CGWindowList)
  accessibility.rs   -- AXRaise によるウィンドウ前面化
  cursor.rs          -- マ���スカーソル移動 (CGEvent)
```
