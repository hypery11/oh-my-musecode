# oh-my-musecode

英文主文件：[README.md](README.md)。

`omm` 是給 **Meta Muse Code** 的精選內容包、輕 CLI 與 git marketplace：35 個技能、3 個斜線指令、8 個 hooks、1 個 reminder、內建 MCP server、3 套 themes、4 個 settings profiles，走 Muse 自己的 plugin 與 config 介面安裝。寫進 Muse 目錄的每個 byte 都有 ownership ledger，`omm uninstall` 整機還原，`omm doctor` 直接告訴你 live session 會踩到哪個靜默失敗。不碰 Muse binary，不寫 framework state 進 `settings.json`，沒有人（或明確 `--yes`）就不寫任何東西。

## 安裝

第一階段只放一個 static binary，其他什麼都不寫。第二階段（`omm install`）是唯一會動 Muse config 的步驟。

```sh
curl -fsSL https://github.com/hypery11/oh-my-musecode/releases/latest/download/install.sh | sh
```

或從 clone 自建：`cargo build --release -p omm`，把 `target/release/omm` 放上 PATH，再 `omm install --source "$PWD"`。

從 0.x（Node CLI、Python hooks、角色技能）升級？先讀 `docs/MIGRATION.md`——1.0.0 是取代，不是延伸。

## 用法

- `/omm-doctor`——診斷工作區與 plugin 安裝。
- `/omm-cost`——開更多技能前先看 catalog 佔多少 context。
- `/omm-status`——`.omm/` 快照。
- 技能核准後由 Muse 自動取用；hooks 在 `muse plugins approve oh-my-musecode` 之後才會跑。

## 限制，先講清楚

- `omm-guard` 是對指令原文的啟發式檢查，不是安全邊界。真正的邊界是 sandbox 與 host 權限。
- stop、subagent、pre-compact 三個 hook 只觀察紀錄，不擋、不否決、不改寫。
- Hook payload 是對 Muse 1.3.0 實測 pin 住的，host 一改 `omm doctor` 就會說。

授權：MIT。Copyright 2026 hypery11。
