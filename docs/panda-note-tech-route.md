我把前一版改成“服务端已存在、三栏 Cloud-native 笔记客户端”的技术说明，给 Agent 时补上服务端路径即可。

# Panda Note 桌面客户端技术路线说明

## 一、项目定位

Panda Note 是一个 **Cloud-native 笔记应用**，整体包含：

* 已完成的服务端；
* 桌面客户端；
* 后续可能扩展的移动端或 Web 端。

服务端依据 EdgeEver 的设计重新实现，已经具备笔记数据管理和同步能力。

服务端代码路径：

```text
Z:\source\lab\everbook\panda-server
```

桌面客户端（Zed fork）代码路径：

```text
Z:\source\lab\everbook\panda-desktop
```

本地构建产物目录（Z 盘空间有限，target 放 D 盘）：

```text
D:\cargo-target\panda-desktop
```

由 `panda-desktop/.cargo/config.toml` 的 `build.target-dir` 指定，勿把该目录提交进 git。

Windows 额外注意：

* 若遇到 git checkout **路径过长**，将 `CARGO_HOME` 设为短路径（例如 `D:\ch`）后再构建。
* 若缺少 Spectre-mitigated MSVC libs，workspace 已用 `crates/msvc_spectre_libs_stub` patch；也可在 VS Installer 安装对应 Spectre 组件后去掉该 patch。

Agent 在设计客户端数据模型、接口调用、同步机制之前，必须先阅读服务端代码和接口定义，不得自行假设服务端的数据结构。服务端字段与 API 摘要见 [panda-server client-adapter](../../panda-server/docs/client-adapter.md)。

本阶段的主要任务是：

> 基于 Zed 的完整编辑器能力，实现一个原生、高性能、三栏布局的 Cloud-native 桌面笔记客户端。

---

## 二、产品核心目标

本项目不是传统的本地 Markdown 文件编辑器，也不是重新开发一套代码编辑器。

客户端的核心特点是：

1. 以服务端数据为主要业务来源；
2. 支持本地缓存和离线编辑；
3. 与服务端进行可靠同步；
4. 采用三栏式笔记界面；
5. 保留 Zed 原生编辑器体验；
6. 支持 Vim、多光标、快捷键、搜索替换、Undo/Redo 等成熟能力；
7. 最终产品界面必须是纯笔记应用，不应保留明显的 IDE 形态。

核心价值是：

> 使用 Zed 的原生编辑能力，构建一个 Cloud-native 的高性能笔记客户端。

---

## 三、确定的技术路线

采用以下路线：

```text
Fork Zed
→ 新建独立 crates/panda
→ 保留 Zed Editor、Vim、Workspace 等核心能力
→ 实现三栏笔记界面
→ 接入现有服务端
→ 增加本地缓存与同步层
→ 隐藏或停止初始化 IDE 无关功能
→ 后期按依赖关系逐步裁剪
```

核心原则：

> 不把 Zed Editor 搬进现有客户端框架，而是在 Zed 技术栈上构建笔记客户端。

---

## 四、明确不采用的方案

禁止采用以下方案：

1. 不使用 `gpui-component` 的 Editor 重新实现编辑器；
2. 不单独复制 Zed 的 `editor` crate 到其他工程；
3. 不尝试把 Zed Editor 强行嵌入原有 `gpui-component` 应用；
4. 不自行重写 Vim、多光标、Selection、Undo、IME 等核心编辑能力；
5. 不在项目初期抽离 Editor、Workspace、Project、Buffer 之间的依赖；
6. 不把客户端设计成单纯的本地 Markdown 文件管理器；
7. 不绕过现有服务端重新设计另一套独立数据模型；
8. 不以删除最多的 Zed 源码为项目目标。

Zed Editor 不是一个独立控件，而是依赖 Workspace、Project、Buffer、Language、Settings、Theme、Action 等体系。

强行抽离会导致大量重复开发，并增加长期同步 Zed 上游的成本。

---

## 五、客户端界面结构

客户端采用固定的三栏结构。

```text
┌────────────────┬────────────────────┬──────────────────────────────┐
│ 第一栏          │ 第二栏              │ 第三栏                        │
│ 导航与分类       │ 笔记列表             │ 编辑器                         │
│                │                    │                              │
│ 全部笔记         │ 当前分类下的笔记      │ Zed Editor                    │
│ 收藏            │ 标题                │                              │
│ 最近访问         │ 摘要                │ Markdown / 富文本化 Markdown   │
│ 文件夹           │ 更新时间             │ Vim / 多光标 / 搜索替换        │
│ 标签            │ 同步状态             │                              │
│ 回收站           │                    │                              │
└────────────────┴────────────────────┴──────────────────────────────┘
```

### 第一栏：导航栏

第一栏负责全局范围和笔记组织方式，包括：

* 全部笔记；
* 收藏；
* 最近访问；
* 文件夹；
* 标签；
* 回收站；
* 可能的共享空间；
* 账号和同步状态入口。

第一栏不直接承担正文编辑。

### 第二栏：笔记列表

第二栏显示当前导航范围内的笔记，包括：

* 标题；
* 摘要；
* 更新时间；
* 收藏状态；
* 标签；
* 同步状态；
* 冲突或失败状态。

需要支持：

* 新建笔记；
* 删除笔记；
* 重命名；
* 搜索；
* 排序；
* 筛选；
* 多选；
* 右键菜单；
* 键盘导航。

### 第三栏：编辑器

第三栏使用 Zed Editor 作为正文编辑核心。

保留：

* 原生文本渲染；
* Vim 模式；
* 多光标；
* 快捷键；
* Undo/Redo；
* 搜索替换；
* Selection；
* IME；
* Markdown 语法高亮；
* 折行；
* 大文本编辑性能。

第三栏还需要显示：

* 笔记标题；
* 保存状态；
* 同步状态；
* 标签或属性入口；
* 冲突提示；
* 可能的 Markdown 预览。

---

## 六、项目结构

在 Zed Cargo Workspace（`panda-desktop`）中新增以下模块。采用与 Zed 一致的**扁平** `crates/` 布局，用 `panda` / `panda_*` 前缀区分；**不**单独开 `crates/panda/{core,api,…}` 或仓库根 `panda/` 子树。

```text
crates/
├── panda/             # Panda Note 桌面客户端入口
├── panda_core/        # 笔记领域模型
├── panda_api/         # 服务端 API 客户端
├── panda_store/       # 本地缓存和持久化
├── panda_sync/        # 同步队列、冲突处理、增量同步
├── panda_index/       # 本地全文索引和搜索
├── panda_ui/          # 三栏布局及笔记业务 UI
└── panda_session/     # 登录状态、工作区和会话恢复
```

继续使用 Zed 原有模块：

```text
gpui
ui
editor
vim
workspace
project
worktree
multi_buffer
language
text
rope
settings
theme
search
picker
command_palette
markdown
```

不要直接把 `crates/zed` 修改成 Panda Note。

必须新建独立应用入口：

```text
crates/panda
```

最终构建目标：

```bash
cargo build --release -p panda
```

构建产物输出到 `D:\cargo-target\panda-desktop`（见 `.cargo/config.toml`）。

---

## 七、服务端优先原则

服务端已经完成，客户端开发必须以现有服务端实现为依据。

开始编码前，Agent 必须完成以下工作：

1. 阅读服务端目录结构；
2. 确认数据库实体；
3. 确认 API 接口；
4. 确认认证方式；
5. 确认笔记、文件夹、标签的数据关系；
6. 确认服务端是否支持增量同步；
7. 确认是否存在版本号、更新时间或 Revision；
8. 确认删除逻辑是软删除还是物理删除；
9. 确认附件上传和访问方式；
10. 确认服务端是否已有 WebSocket、SSE 或变更通知机制。

服务端路径：

```text
Z:\source\lab\everbook\panda-server
```

不得在未阅读服务端代码的情况下，直接创建另一套不兼容的客户端模型。

---

## 八、建议的数据分层

客户端不得直接将 UI 与 HTTP API 绑定。

采用以下分层：

```text
UI
↓
panda_core / application service
↓
local repository
↓
sync engine
↓
remote API
```

### 1. 远程数据层

由 `panda_api` 负责：

* 登录；
* Token 管理；
* 拉取笔记；
* 创建笔记；
* 更新笔记；
* 删除笔记；
* 文件夹和标签操作；
* 附件上传；
* 增量同步；
* 获取服务端变更。

### 2. 本地数据层

由 `panda_store` 负责：

* 本地笔记缓存；
* 本地编辑状态；
* 离线数据；
* 待同步操作；
* 最近访问；
* UI 状态；
* 搜索索引；
* 失败重试记录。

本地建议使用 SQLite。

SQLite 是客户端本地缓存和离线数据库，不是替代服务端数据库。

### 3. 同步层

由 `panda_sync` 负责：

* 首次全量同步；
* 后续增量同步；
* 本地修改上传；
* 服务端修改拉取；
* 离线操作队列；
* 网络恢复后重试；
* 删除同步；
* 同步状态维护；
* 冲突检测；
* 冲突解决。

---

## 九、客户端数据状态

每一篇笔记至少应具有明确的本地同步状态。

```rust
pub enum SyncState {
    Synced,
    LocalCreated,
    LocalModified,
    LocalDeleted,
    Syncing,
    Conflict,
    Failed,
}
```

建议的客户端笔记模型：

```rust
pub struct Note {
    pub id: NoteId,
    pub remote_id: Option<String>,
    pub title: String,
    pub content: String,
    pub folder_id: Option<FolderId>,
    pub tags: Vec<TagId>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub remote_revision: Option<String>,
    pub sync_state: SyncState,
}
```

具体字段必须根据服务端代码调整，不得直接照搬此示例。

---

## 十、编辑与保存策略

编辑器输入不能每输入一个字符就立即发送网络请求。

建议流程：

```text
用户输入
→ 更新 Zed Buffer
→ 短间隔防抖
→ 写入本地 SQLite
→ 标记 LocalModified
→ 后台同步队列上传服务端
→ 上传成功
→ 标记 Synced
```

需要区分三个状态：

```text
Editor Buffer 状态
本地数据库状态
远程服务端状态
```

客户端应优先保证：

1. 用户输入不丢失；
2. 本地保存不依赖网络；
3. 网络同步失败不阻塞编辑；
4. 同步恢复后自动重试；
5. 关闭应用前本地数据已经落盘。

---

## 十一、Editor 集成原则

不要修改 Zed Editor 的核心文本编辑逻辑。

在外层增加笔记包装：

```rust
pub struct NoteEditor {
    pub editor: Entity<Editor>,
    pub note_id: NoteId,
    pub sync_state: Entity<SyncState>,
    pub metadata: Entity<NoteMetadata>,
}
```

`NoteEditor` 负责：

* 当前笔记 ID；
* 标题；
* 文件夹；
* 标签；
* 本地保存；
* 远程同步；
* 同步状态；
* 冲突提示；
* Markdown 预览；
* 笔记属性。

以下能力全部交给 Zed Editor：

* 文本输入；
* 光标和选区；
* 多光标；
* Vim；
* Undo/Redo；
* 快捷键；
* 搜索替换；
* IME；
* 折行；
* Tree-sitter；
* 文本渲染。

---

## 十二、Buffer 与服务端笔记的关系

Zed Editor 通常围绕 Buffer 和文件系统工作，但本项目是 Cloud-native 笔记。

不要为了适配服务端而重写 Editor。

优先采用以下策略之一：

### 推荐方案：本地镜像文件或本地 Buffer 适配层

```text
服务端 Note
↓
本地缓存
↓
本地 Markdown 镜像或 Buffer
↓
Zed Editor
```

编辑器只负责编辑本地 Buffer。

`panda_store` 和 `panda_sync` 负责把 Buffer 的内容保存到本地数据库并同步至服务端。

是否需要真实的本地 Markdown 镜像文件，应根据 Zed Buffer 的接入成本决定。

优先目标是：

* 最大限度复用 Zed Buffer；
* 最少修改 Editor；
* 不让网络请求进入 Editor 核心；
* 不让远程数据模型污染 Zed 核心模块。

---

## 十三、冲突处理

Cloud-native 客户端必须考虑多端编辑冲突。

在确认服务端实现后，选择适合的冲突策略。

最低要求：

```text
上传前比较 remote_revision
↓
版本一致：正常更新
版本不一致：进入 Conflict
```

出现冲突时不得静默覆盖。

可以提供：

* 保留本地版本；
* 使用远程版本；
* 创建冲突副本；
* 打开差异对比；
* 手动合并。

第一阶段可以先采用：

> 检测到冲突后生成一篇冲突副本，避免任何内容丢失。

后期再实现更完整的 Diff 和合并界面。

---

## 十四、第一阶段开发范围

第一阶段目标是完成可用闭环，不追求所有高级功能。

### 必须完成

```text
启动应用
→ 登录服务端
→ 拉取笔记数据
→ 展示三栏界面
→ 导航分类
→ 展示笔记列表
→ 打开笔记
→ 使用 Zed Editor 编辑
→ 本地自动保存
→ 上传服务端
→ 展示同步状态
```

同时支持：

* 新建笔记；
* 编辑笔记；
* 删除笔记；
* 收藏；
* 文件夹切换；
* 标签显示；
* 搜索；
* 基础离线编辑；
* 网络恢复后重试。

### 第一阶段暂不要求

* 实时多人协同；
* CRDT；
* 完整历史版本；
* 富文本所见即所得；
* 复杂附件编辑；
* 移动端同步；
* 插件市场；
* AI Agent；
* Zed 原有 IDE 功能。

---

## 十五、Zed 概念映射

可以复用 Zed 的基础设施，但不能让产品呈现 IDE 语义。

```text
Zed Workspace       → Panda Note 主窗口
Zed Pane            → 编辑区域
Zed Editor Item     → 当前笔记编辑视图
Zed Buffer          → 笔记正文编辑缓存
Zed Search          → 笔记内搜索或全局搜索基础
Zed Outline         → Markdown 大纲
Zed Settings        → 客户端设置
Zed Theme           → 客户端主题
```

由于笔记主体来自服务端，不能简单地将：

```text
Project = 服务端全部数据
```

`Project` 和 `Worktree` 是否保留为内部适配基础，应根据实际 Editor 初始化依赖决定，不需要强行暴露给用户。

---

## 十六、无关功能处理方式

第一阶段不要直接删除 Zed 源码。

先不注册或不展示：

```text
Agent
Terminal
Git
Debugger
Collaboration
Remote Development
Extension Marketplace
REPL
Tasks
Copilot
Language Model
Project Panel
IDE Status Bar
```

裁剪顺序必须是：

```text
隐藏 UI
→ 停止注册 Action
→ 停止初始化模块
→ 删除 panda 的直接依赖
→ 最后清理 Cargo 依赖
```

不要一开始批量删除 Zed crates。

---

## 十七、裁剪目标

本项目的裁剪目标是：

1. 产品界面是纯三栏笔记应用；
2. 无关 IDE 模块不初始化；
3. 无关功能不进入发布版本；
4. 客户端启动速度和资源占用合理；
5. 保持 Zed 核心改动尽可能少；
6. 后续能够同步 Zed 上游更新。

不要求：

* 源码仓库只剩几个 crate；
* 完全移除 Workspace、Project；
* 将 Editor 抽成独立通用组件；
* 删除所有 IDE 命名；
* 让整个 Zed 仓库变得非常小。

---

## 十八、代码修改原则

必须遵守：

```text
尽量不修改 gpui
尽量不修改 editor
尽量不修改 vim
少修改 workspace
少修改 project
业务功能全部放入 panda_* crates
```

确实需要修改 Zed 原模块时：

* 修改范围尽量小；
* 每项修改单独提交；
* 优先暴露接口，不重写原逻辑；
* 不在 Zed 核心模块中直接加入服务端业务代码；
* 不让 `editor` 直接依赖 `panda_api` 或 `panda_sync`。

合理的修改示例：

```text
patch: expose editor constructor
patch: support custom buffer source
patch: expose buffer changed event
patch: support custom workspace item
patch: disable coding service initialization
```

不合理的修改示例：

```text
editor directly calls note server
editor stores authentication token
workspace directly parses note API response
vim action directly updates remote note
```

---

## 十九、最终技术栈

```text
语言：
Rust

桌面 UI：
GPUI + Zed UI + Zed Theme

编辑器：
Zed Editor + Zed Vim + MultiBuffer + Tree-sitter

界面：
三栏笔记布局

远程服务：
现有 EdgeEver 重新实现版服务端

本地缓存：
SQLite

本地搜索：
SQLite FTS5
后续根据数据规模评估 Tantivy

网络：
根据服务端实现选择 HTTP
配合 WebSocket 或 SSE 接收变更通知

同步：
本地优先保存
操作队列
增量同步
失败重试
版本冲突检测
```

---

## 二十、Agent 执行顺序

Agent 应严格按照以下顺序执行。

### 阶段 1：服务端分析

```text
读取服务端代码
→ 整理实体模型
→ 整理 API
→ 整理认证机制
→ 整理同步和版本机制
→ 输出客户端适配方案
```

### 阶段 2：Zed 最小验证

```text
新建 crates/panda
→ 启动 GPUI 窗口
→ 初始化 Zed Editor
→ 加载一个测试 Markdown
→ 验证 Vim 和基本编辑
```

### 阶段 3：三栏界面

```text
实现第一栏导航
→ 实现第二栏笔记列表
→ 第三栏嵌入 Zed Editor
→ 完成笔记切换
```

### 阶段 4：本地数据

```text
建立 SQLite
→ 缓存远程笔记
→ 编辑内容本地落盘
→ 恢复上次打开状态
```

### 阶段 5：服务端接入

```text
登录
→ 拉取数据
→ 创建和更新笔记
→ 删除笔记
→ 同步状态展示
```

### 阶段 6：离线与冲突

```text
离线编辑
→ 操作队列
→ 自动重试
→ Revision 校验
→ 冲突副本
```

### 阶段 7：功能裁剪

```text
隐藏 IDE UI
→ 停止无关模块初始化
→ 清理 panda 依赖
→ 优化启动速度和安装体积
```

---

## 二十一、最终执行原则

开发过程中始终遵循：

> 服务端已有能力优先复用，不重新设计一套不兼容协议。

> 编辑内容必须先可靠地保存到本地，再进行远程同步。

> 能复用 Zed 的编辑能力，就不要重新实现。

> 能在外层包装，就不要修改 Editor 核心。

> 网络层、同步层和编辑器核心必须解耦。

> 优先完成三栏客户端和完整同步闭环，再进行深度裁剪。

服务端路径补进去后，这份可以直接作为 Agent 的项目级说明。
