<div align="center">

# Claude 额度监控 · claude-limit

**实时监控 Claude 订阅版（Pro / Max）的 5 小时 / 7 天滚动额度与用量的 Windows 桌面应用。**

[![License: MIT](https://img.shields.io/badge/License-MIT-coral.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)
![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%20v2-24C8DB)

</div>

> ⚠️ 订阅版没有官方“剩余额度”接口，本应用的额度数字均为**本地估算值**；额度上限按套餐档位估计并随历史用量自动校准，可能与真实限额有偏差。可选启用「官方利用率源」获取 Anthropic 的官方利用率百分比（只读，不消耗额度）。

<!-- 截图：把图片放到 docs/ 下后取消注释（强烈建议补 1~2 张仪表盘截图，社区最先看的就是这个） -->
<!--
<div align="center">
  <img src="docs/dashboard.png" width="720" alt="仪表盘" />
</div>
-->

## 这是什么 / 为什么

用 Claude Code 的人常困惑：“我这 5 小时还剩多少额度？会不会马上触顶？”订阅版不像 API 那样有明确计量，官方也不暴露剩余额度。

本应用读取你本机的 Claude Code 用量日志（`~/.claude/projects/**/*.jsonl`），在本地估算 5 小时 / 7 天滚动窗口的用量、消耗速率与触顶时间，并在接近额度时弹 Windows 通知——**全程不联网上传任何数据**。

## 🔒 隐私与安全（请先读这一段）

- **本地优先**：用量统计全部基于你本机已有的 Claude Code 日志，解析与计算都在本地完成。
- **OAuth token 绝不外泄**：套餐档位读自 `~/.claude/.credentials.json`；其中的 OAuth token **仅在内存中使用，绝不持久化、绝不打印、绝不传给前端**。
- **不抢登录态**：应用**不会**自行刷新你的 OAuth token——token 过期时它跳过请求并提示“请在 Claude Code 中活动以刷新”，避免轮换 refresh token 破坏你的 Claude Code 登录。
- **唯一的可选联网**：仅当你在设置里**主动开启**「官方利用率源」时，才会用本地 token 调用 Anthropic 的**只读**用量接口（不消耗额度）；默认关闭，关闭时完全离线。

## 功能

- **实时仪表盘**：5h / 7d 利用率环形图、已用 / 剩余、消耗速率、预计触顶时间
- **历史趋势**：按天 / 按小时的用量堆叠图（按模型）
- **细分统计**：按模型（Opus / Sonnet / Haiku）、按项目
- **额度告警**：越过阈值（默认 75 / 90 / 100%）弹 Windows 通知（去重，窗口重置后复位）
- **系统托盘**：动态饼图图标显示 5h 利用率，悬停看百分比；关闭窗口 = 最小化到托盘
- **开机自启**、浅 / 深色主题（跟随系统）

## 安装

### 方式一：下载安装包（推荐）

到 [Releases](../../releases) 下载最新版的安装包（`*-setup.exe`），双击安装即可（含开始菜单快捷方式与卸载程序）。

### 方式二：便携版

如提供便携单文件 `.exe`，直接双击运行，无需安装。

**运行要求**：Windows 10 / 11，且已安装 **Microsoft Edge WebView2 运行时**（Win11 自带，Win10 一般也已预装；如缺失可从微软官网安装）。

> 未签名的可执行文件首次运行可能被 SmartScreen 拦截，点“更多信息 → 仍要运行”即可。

## 工作原理与准确性

- 套餐档位自动识别自 `~/.claude/.credentials.json`（`subscriptionType` / `rateLimitTier`）。
- 用量明细来自逐条 assistant 消息的 `usage`（input / output / cache_creation / cache_read）。
- **去重按 `message.id:requestId`**（与 [ccusage](https://github.com/ryoppippi/ccusage) 一致），避免同一响应在续聊 / 分叉会话文件中被重复计数；重复时保留 token 总量最大的副本。
- 额度计权默认 `input + output + cache_creation`（`cache_read` 默认权重 0），可在设置中调整。
- **交叉验证**：与 `npx ccusage daily` 对比，已完成日期的 token 总量基本逐项吻合。

## 官方利用率源（可选）

在「设置」启用后，应用用本地 OAuth token 调用 Anthropic 的**只读用量接口**获取官方 5h / 7d 利用率：

```
GET https://api.anthropic.com/api/oauth/usage
Authorization: Bearer <accessToken>
anthropic-beta: oauth-2025-04-20
→ { "five_hour": {utilization, resets_at}, "seven_day": {utilization, resets_at}, ... }
```

- **不消耗额度**（只读），后台每约 3 分钟刷新，429 时退避 10 分钟。
- 启用后仪表盘显示**官方百分比与官方重置时间**，并据 `已用 / 利用率` 反推校准上限；失败自动回退本地估算并给出说明。

## 技术栈

- **后端**：Rust + Tauri v2（定时轮询增量读取 JSONL；`rusqlite` bundled SQLite；托盘 / 通知 / 自启插件）
- **前端**：React 19 + Vite + Tailwind v4 + ECharts（Anthropic 风格暖色主题）

## 从源码构建

> 需要 Rust（MSVC 工具链，已在 `rust-toolchain.toml` 固定为 1.88）、Node.js、WebView2。

```bash
npm install
npm run tauri dev      # 开发运行（热重载）
npm run tauri build    # 打包 Windows NSIS 安装包
```

数据库位于 `%APPDATA%\com.claudelimit.monitor\usage.db`。

### 发布（CI）

仓库内置 GitHub Actions（[`.github/workflows/release.yml`](.github/workflows/release.yml)）：推送 `v*` tag 时在**干净的 Windows runner** 上构建 NSIS 安装包并创建**草稿 Release**。这同时绕开了本机“项目在 E: 盘、tauri 工具缓存在 C: 盘”导致的跨盘 `rename`（os error 17）打包失败。

```bash
git tag v0.1.0 && git push origin v0.1.0   # 触发自动构建与草稿 Release
```

## 目录结构

```
src-tauri/src/
  creds.rs       # 读取 credentials、套餐档位映射
  ingest.rs      # 增量扫描 JSONL + 解析 + 去重
  store.rs       # SQLite：事件 / 偏移 / 设置 / 聚合查询
  aggregate.rs   # 5h/7d 窗口、burn rate、预测 -> Status
  tray.rs        # 动态饼图托盘图标 + 菜单
  alerts.rs      # 阈值通知（去重）
  official.rs    # 官方利用率源（只读）
  commands.rs    # Tauri IPC 命令
  lib.rs         # 装配、后台轮询 tick、窗口事件
src/
  pages/         # Dashboard / History / Breakdown / Settings / Log
  components/    # Gauge / Chart / ui
  lib/           # ipc.ts（类型 + 命令封装）/ format.ts
```

## 路线图

- 把官方利用率历史也落库，绘制官方 vs 本地估算偏差曲线。
- token 临近过期时主动提示用户。

## 致谢

去重计量口径参考了 [ccusage](https://github.com/ryoppippi/ccusage)。

## License

[MIT](LICENSE) © fengdiqing
