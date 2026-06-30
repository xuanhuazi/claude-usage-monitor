# Claude 额度监控 (claude-limit)

一个 **Windows 桌面应用**，实时监控 Claude **订阅版（Pro/Max）** 的额度与用量。
基于本地 Claude Code 日志（`~/.claude/projects/**/*.jsonl`）估算 5 小时 / 7 天滚动窗口用量、消耗速率与触顶预测，并在接近额度时桌面告警。

> ⚠️ 订阅版没有官方“剩余额度”接口，本应用的额度数字均为**本地估算值**；额度上限按套餐档位估计并随历史用量自动校准，可能与真实限额有偏差。

## 功能

- **实时仪表盘**：5h / 7d 利用率环形图、已用 / 剩余、消耗速率、预计触顶时间
- **历史趋势**：按天 / 按小时的用量堆叠图（按模型）
- **细分**：按模型（Opus/Sonnet/Haiku）、按项目统计
- **额度告警**：越过阈值（默认 75/90/100%）弹 Windows 通知（去重、窗口重置后复位）
- **系统托盘**：动态饼图图标显示 5h 利用率 + 悬停查看百分比；关闭窗口最小化到托盘
- **开机自启**、浅/深色主题（跟随系统）

## 数据与准确性

- 套餐档位自动识别自 `~/.claude/.credentials.json`（`subscriptionType` / `rateLimitTier`）。**OAuth token 仅在内存使用，绝不持久化或暴露给前端。**
- 用量明细来自逐条 assistant 消息的 `usage`（input / output / cache_creation / cache_read）。
- **去重按 `message.id:requestId`**（与 ccusage 一致），避免同一响应在续聊/分叉会话文件中被重复计数；重复时保留 token 总量最大的副本。
- 额度计权默认 `input + output + cache_creation`（`cache_read` 默认权重 0），可在设置中调整。
- 交叉验证：与 `npx ccusage daily` 对比，已完成日期的 token 总量基本逐项吻合（缓存类 ~0%）。

## 技术栈

- **后端**：Rust + Tauri v2（`notify` 之外采用定时轮询增量读取；`rusqlite` bundled SQLite；托盘 / 通知 / 自启插件）
- **前端**：React 19 + Vite + Tailwind v4 + ECharts；字体 Fraunces / Inter（Anthropic 风格暖色主题）

## 开发与构建

> 需要 Rust（MSVC 工具链，已在 `rust-toolchain.toml` 固定）、Node.js、WebView2。

```bash
npm install
npm run tauri dev      # 开发运行（热重载）
npm run tauri build    # 打包 Windows 安装包
```

数据库位于 `%APPDATA%\com.claudelimit.monitor\usage.db`。

## 目录结构

```
src-tauri/src/
  creds.rs       # 读取 credentials、套餐档位映射
  ingest.rs      # 增量扫描 JSONL + 解析 + 去重
  store.rs       # SQLite：事件 / 偏移 / 设置 / 聚合查询
  aggregate.rs   # 5h/7d 窗口、burn rate、预测 -> Status
  tray.rs        # 动态饼图托盘图标 + 菜单
  alerts.rs      # 阈值通知（去重）
  official.rs    # 官方利用率源（实验，待验证）
  commands.rs    # Tauri IPC 命令
  lib.rs         # 装配、后台轮询 tick、窗口事件
src/
  pages/         # Dashboard / History / Breakdown / Settings
  components/     # Gauge / Chart / ui
  lib/           # ipc.ts (类型 + 命令封装) / format.ts
  index.css       # 设计 token（暖色 + 珊瑚橙，浅/深主题）
```

## 官方利用率源（Phase 4，已实现）

在「设置」启用后，应用用本地 OAuth token 调用 Anthropic 的**只读用量接口**获取官方 5h/7d 利用率：

```
GET https://api.anthropic.com/api/oauth/usage
Authorization: Bearer <accessToken>
anthropic-beta: oauth-2025-04-20
User-Agent: claude-code/<ver>
→ { "five_hour": {utilization, resets_at}, "seven_day": {utilization, resets_at}, ... }
```

- **不消耗额度**（只读），后台每约 3 分钟刷新，429 时退避 10 分钟。
- 启用后仪表盘显示**官方百分比与官方重置时间**，并据 `已用 / 利用率` 反推校准上限；失败自动回退本地估算并给出说明。
- **不自行刷新 OAuth token**：token 过期时跳过请求并提示「请在 Claude Code 中活动以刷新」，避免轮换 refresh token 破坏 Claude Code 登录。token 由 `read_oauth()` 仅在内存读取，绝不落盘/打印/传前端。

## 分发

- **便携版（已产出）**：`dist-portable\Claude额度监控.exe` —— release 模式下前端已内嵌进 exe，单文件即可运行（需系统已装 WebView2），无需安装。自带品牌图标。
- **安装包（NSIS/MSI）**：在本机打包时因「项目在 E: 盘、而 tauri 工具缓存/临时目录在 C: 盘」触发跨盘 `rename`（os error 17）而失败。在「项目目录、临时目录、`%LOCALAPPDATA%\tauri` 工具缓存同处一个磁盘卷」的环境下执行 `npm run tauri build` 即可正常产出安装包。

## 路线图

- 可选：把官方利用率历史也落库，绘制官方 vs 本地估算偏差曲线。
- 可选：token 临近过期时主动提示用户。
