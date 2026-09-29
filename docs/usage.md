# Carbot 使用与配置

安装和架构见 [README](../README.md)。

## Web 与 CLI

Web 使用 Vue 3 + Element Plus。终端与 Web 共享同一应用层、Agent、项目、日志和权限审批，不依赖彼此转发管理命令。

### 项目就是聊天群组

每个项目配置成员、角色、工作目录和协作策略：

| 模式 | 行为 |
| --- | --- |
| 简单聊天 | 有且仅有一个 Agent |
| A2A | 按轮次让成员共享上下文交流 |
| PMO | Leader 按成员角色分配任务并汇总 |
| 接力 | 按指定顺序、随机或协商优先级选择 Agent；额度不足时交给下一个 |

项目名称可留空，首次消息生成初始标题。列表「⋯」支持重命名和删除。删除采用软删除：从列表移除、禁止继续发送，本地历史日志保留；不删除 Agent。运行中的任务需先停止。

消息按连续聊天展示日期、时间和 Markdown。向上滚动加载较早消息；执行者与进度不拼进回答正文。工具调用可展开查看详细参数与结果。

### Agent 管理

「管理」中维护本地 Agent、虚拟 Agent 和组网。默认 Agent 不能删除；普通本地 Agent 编辑、删除前先停用，被项目引用时不能删除。停用只停止接收新任务，不打断已经开始的任务；取消任务请用聊天中的停止生成或 /interrupt。

远端 Agent 用「远端」标记，仅可查看和测试，不可在本节点改配置。IP:端口标注为连接来源，可能是 NAT / 代理后的地址，**不是远端 Web 管理地址**。

编辑角色与测试聊天使用右侧抽屉；测试会话单独持久化。远端目录在后台发现，不阻塞本地 Agent 操作。

### Tool 与 Skill

定义统一维护，管理能力与项目能力隔离。内置工具只能启用/禁用；非硬编码外部命令与 Skill 支持新增、编辑、上传、删除及测试。

每一行有「测试运行」和「配置范围」：
- 全局只列本地 Agent；选择项目后仅列该项目的本地成员。
- 不在 Tool / Skill 配置与测试中列远端 Agent；远端能力在远端节点维护。
- Agent 表格直接开关启用，自动保存，下一轮生效。
- 所有项目设置的是 Agent 默认值；单个项目设置可覆盖它。底层仍兼容全局、项目、Agent、项目内 Agent 的层级规则。

启用能力不是授权。命令、目录外访问、Python 仍受宿主策略和人工审批约束；管理 Agent 不能批准自己的请求。

### 执行权限

本地 Agent 支持三档，默认 `ask`，只由 CLI/Web 人工入口修改，不注册为管理 Agent 工具。权限保存在本节点该 Agent 的配置中，作用于该 Agent 的所有项目，不传递给远端 Agent。管理聊天独立保留审批，配置更新只影响后续任务；降低权限不会终止正在运行的任务，必要时先停止生成。

- **请求批准（ask）**：Carbot Shell 命令每次询问。
- **帮我批准（auto）**：使用当前 Carbot 实例全局命令白名单，默认约 50 条常见只读、目录查询、版本及受限 Git 查询命令。匹配完整命令和参数，支持 `*`（任意字符，含多个参数）和 `?`（单个字符），例如 `git status *`、`cat *.md`。可执行文件名必须明确；仍拒绝管道、重定向及命令替换。宽泛规则可放行脚本和危险参数，请谨慎配置。自定义脚本命令依然可能危险，请自行核对。
- **完全访问（full）**：明确确认后取消执行确认和目录沙箱，可访问宿主当前用户有权限的文件和网络，包括目录外及凭据。仍不提供系统管理员权限，不自动启用被禁用的 Tool/Skill/Python。仅用于可信任务。

Web 在单本地 Agent 项目输入区域切换，完全访问有独立风险确认框。CLI：

```text
/permissions                         查看当前单 Agent 项目权限
/permissions default                 查看指定 Agent
/permissions default ask
/permissions default auto
/permissions default full --confirm-full-access
/allowlist                            查看全局白名单
/allowlist add cat README.md
/allowlist remove cat README.md
/allowlist reset                      恢复默认列表
/allowlist clear                      清空列表
```

普通读写 HTTP 接口：`GET/PUT /v1/repl/{project}/agents/{id}/permissions`；写入需 `mode`、`expected_version`，full 另需 `confirm_full_access:true`。沿用本机管理接口的信任边界，请勿对不可信本地程序开放管理端口；变更有审计记录。普通 Agent 配置保存不能注入权限字段。

**原生运行器边界**：Codex/Claude 非交互命令的逐工具授权协议尚未转接。ask/auto 下分别保留 Codex read-only / Claude plan，需执行命令时走 Carbot 注册工具及审批；不能转接的原生操作会拒绝，不会默认批准。full 下使用 Codex danger-full-access / Claude bypassPermissions，并取消 Carbot 外层目录沙箱。此设置不解决模型账号登录认证问题。

授权与命令结果在 Web 输入框上方展示；默认只展示操作描述和目标，完整命令可展开核对。白名单在「管理 → 权限白名单」独立页面维护，GET/PUT /v1/permissions/allowlist，写入包含 command_allowlist 字符串数组和 expected_version。所有本地 Agent 共用，配置持久化并记录审计，不提供给模型自我修改。

命令请求支持「允许一次」「本对话允许」「拒绝」。本对话允许仅跳过当前项目聊天后续命令确认，不扩大目录访问权限；重置上下文或重启进程后失效，不影响其他聊天和管理操作。

Python 和命令执行保留文件系统沙箱，网络固定使用宿主网络，旧执行 profile 的 network:none 不再用于它们；常见 HTTP(S)/ALL/NO_PROXY 环境变量会透传。工作目录不是副本，修改直接落在本机文件上。原生 Codex/Claude 的运行限制仍遵循各自权限模式。

Agent 可配置回复要求（默认简洁、结论先行），作为每次调用的 Agent 提示词，不是硬性字数截断。群聊按 Agent 与 invocation 分开发言，工具事件不再作为回答文本输出；原有日志保持不变。日志文件可在「本地存储 · 排查信息」点击打开右侧抽屉，按 128 KB 分段读取，支持格式化 / JSONL 原文。

组网仅对已有明确 Web 地址提供跳转，不猜测下游来源端口，不提供手动管理地址设置。当前无认证管理接口仍只允许 loopback；跨机管理需本地转发或后续受认证的代理机制。

### 上下文与压缩

Web 聊天右上角「新上下文」与 CLI `/new` 使用同一分界机制：保留项目成员、策略和全部原始 JSONL 日志，但后续请求不再携带分界前的聊天内容。管理聊天也支持；执行中需先停止或等待。分界持久化，重启后仍生效。`/history` 仍只展示历史，不撤销分界。

Carbot Harness 的 Agent 编辑表单支持上下文预算与压缩策略；默认 Agent 在其模型配置中修改，普通 Agent 可继承或独立配置：

- `CONTEXT_MAX_TOKENS`：默认 65536，范围 8192–2000000。采用保守的 UTF-8 字节数估算，不是模型专用 tokenizer；应设为不超过所用模型窗口。实际请求包含系统提示、工具 schema，并预留输出预算（默认 4096）。
- `CONTEXT_STRATEGY=extractive`：默认，本地首尾摘录。
- `CONTEXT_STRATEGY=window`：保留近期片段。
- `CONTEXT_STRATEGY=disabled`：禁用压缩，超限提示新上下文或调整预算。

两种压缩都是有损文本摘录，不是额外调用模型生成的语义摘要。当前请求、管理指引及系统提示不会主动截断；单条当前请求或工具定义本身过大仍需缩短或增加预算。压缩在完整工具轮次之间执行，避免留下孤立的工具调用 ID；日志不被覆盖。上述预算配置作用于 Carbot Harness；Codex/Claude 原生模型上下文管理仍由各自 CLI 负责，`/new` 的 Carbot 历史分界对所有运行器生效。

Rust 采用 `CompressionStrategy` 接口、`CompressionFactory` 工厂，具体策略各自独立文件，`ContextBudget` 负责预算编排。

### CLI 常用操作

```text
/help                       查看分类帮助
/manage 或 /admin           进入管理聊天
/back                       返回上一个聊天
/project                    列出项目
/project GROUP_ID           进入项目
/history                    加载当前聊天历史
/new                        新上下文，保留当前项目与历史
/agent list                 查看本地与远端 Agent
/agent test PATH            测试 Agent
/agent-config               修改默认 Agent 配置
/connections                查看上游与接入下游
/connect URL                申请连接上游
/disconnect upstream NAME   断开上游；downstream 操作接入下游
/reconnect upstream NAME    手动尝试连接；downstream 解除接入限制
/server start 8787          启动 Server
/server stop                只停止 Server
/interrupt                  中断任务
/tools [序号]               查看工具调用详情
/exit                       退出
```

CLI 启动不自动打印历史，使用 `/history` 恢复。鼠标保留终端滚动与拖选复制，↑/↓ 和 PgUp/PgDn 切换输入历史，Tab 补全命令。Ctrl+C 优先中断任务或清空输入；管理会话空闲、输入为空时返回上一个聊天。Ctrl+D 退出。

## 去中心化组网

每个节点既能提供 Agent，也能主动连接上游。下游主动建立 HTTP + SSE 长连接，因此不需要暴露本地公网端口。不同局域网可共同挂载一个公网 Carbot / ProxyAgent。

连接需本机确认授权；自动注册取得 AK/SK，凭据保存于各自节点。上游可发现、调用后代能力及管理授权范围内的子群策略，不能修改远端 Agent 定义。节点不能借此向上访问父节点或兄弟节点的数据。

组网页面展示实际连接配置和状态，不是日志：
- 分别展示上游和接入下游；两端都可断开。
- 每次只尝试连接一次，失败或断线后不自动重试，需手动操作。
- 已连接地址重复提交不会建立重复连接。
- 断开状态持久化；失败连接可删除。下游解除限制后，需要对方手动重试。
- 断开不自动取消已下发任务；聊天和任务日志保留。

## 数据、安全与配置

配置、凭据、策略使用本地状态文件；聊天按 session / 群组 ID，以小时分片的 JSONL 逐行追加，不整份聊天重写。目录锁防止多进程写同一个实例。备份前停止实例，再备份完整数据目录。

`default-agent.json` 保存默认运行器/模型配置；密钥本地明文保存，Unix 文件权限为 0600。不要提交、公开或同步活动实例目录。

无需 AdminToken。管理路由仅允许实际回环连接并校验 Host/Origin；公网仅用于 A2A。远程管理使用 SSH 本地端口转发，不要把管理路由通过反向代理开放到公网。

项目可覆盖工作目录和目录外访问策略（deny / ask），不能扩大宿主启动权限。macOS 使用原生目录隔离，Linux 使用 bubblewrap；不能把仅改变 cwd 当作安全边界。网络按沙箱策略允许，执行权限仍需审批。

模型环境变量：`MODEL_PROVIDER`、`MODEL_NAME`、`MODEL_API_KEY`、`MODEL_BASE_URL`、`MODEL_API`。显式环境变量在重启时优先于保存配置。源码和安装版统一读取 `<实例目录>/.agent.env`（默认 `~/.carbot/.agent.env`）；若运行目录存在 `.agent.env`，其同名配置覆盖实例文件。启动参数优先于显式环境变量，显式环境变量优先于文件。配置文件仅支持字面量 `KEY=value`，不执行 Shell 或变量替换；相对目录基于配置文件所在目录。`--name` / `--data-dir` 先选择实例，再读取配置；实例文件不能通过 `CARBOT_DATA_DIR` 重定向自身。运行目录配置中的 `CARBOT_DATA_DIR` 可以选择实例，仍受显式参数/环境变量覆盖。交互保存的模型配置 `default-agent.json` 优先于文件中的模型默认值，避免旧模板覆盖已保存的配置。

### 本地持久化与内置 Skill

聊天记录写入按小时分片的 JSONL；实例状态写入 `state.jsonl`，一行对应一次事务，仅包含变化的记录，不再在每次操作时重写整份 `state.json`。启动时回放日志，兼容旧 `state.json` 基线；旧文件请保留，不能单独删除。JSONL 中间损坏会拒绝启动，崩溃留下的未完成末行会恢复到最后一次完整提交。日志尚不自动压缩归档，文件会随使用增长。

内置 Skill 源码在 `skills/system/{business,management}/<skill>/`，每个目录包含 `SKILL.md`、`skill.json`（描述和默认启用状态），可附带 `scripts/`、`references/`。`scripts/package-release.sh` 自动将其放入发布包 `skills/system`；`install.sh` 随程序复制到版本目录，启动器通过 `CARBOT_SYSTEM_SKILLS_DIR` 定位。也可显式指定这个环境变量覆盖来源。

系统 Skill 在工具库中只读，可通过已有生效范围规则启用/禁用；安装升级不写入用户 Skill 或实例数据。内置依赖安装、浏览器自动化、OCR Skill 只提供按需工作流，不会在安装 Carbot 时自动安装 Homebrew、浏览器或 Tesseract，也不会绕过执行确认。
