# Skill 与原生目录沙箱

两套能力包共用 Tool / Skill 框架，加载不同实例：
- AdminAgent：management_skills + 内置 management-guide，仅管理工具与 Skill 读取器。
- 业务 Agent：项目 skills + 文件/受控 Python 工具，没有管理工具。

工具名称就是宏声明的注册名，例如 skill_read、skill_file、python_run。没有旧名称转换或额外手写注册表。
读取 Skill 不代表授权运行代码。Python 需要本次 ToolSession 已读取对应 Skill、宿主开关开启、Skill 自身 allow_python 为真，以及执行 profile 在宿主白名单内。

## 启动策略

```bash
export CARBOT_ALLOW_SKILL_PYTHON=1
export CARBOT_SANDBOX_PROFILES_JSON='[{"id":"default","network":"host","timeout_seconds":30}]'
export CARBOT_SANDBOX_PROFILE=default
export CARBOT_SANDBOX_ALLOWED_PROFILES=default
./carbot --workdir /absolute/project --outside-access ask
```

Python 可通过 AGENT_PYTHON_BIN 指定，默认解析 python3。Profile 只保留原生后端真实支持的字段：id、network、timeout_seconds、secret_env。废弃字段会报错，不能继续传 image / runtime / memory_mb / cpus / pids / scratch_mb。

network 为 host 或 none；默认允许网络，不会因为启用目录隔离就自动断网。默认没有注入任何秘密环境变量。
timeout_seconds 为 1..120。最多四个脚本并发，输出上限 64 KiB，CPU 时间和单文件写入大小另受进程限制；这些限制不是容器资源配额或整棵子进程树的内存限额。

## 目录规则

工作目录读写由原生 OS 沙箱限制。macOS 使用 sandbox-exec；Linux 使用 bubblewrap/user namespace。系统运行库、解释器和启动需要的精确祖先目录有只读例外。
Python 的 cwd 是指定工作目录；Skill 包先暂存于隔离运行目录，再以绝对路径执行。临时 HOME 与 TMPDIR 不使用真实用户目录。
宿主数据目录被保护，避免业务 Agent 直接读取管理密钥或历史数据库文件。

目录外路径默认拒绝。ask 模式下，统一文件工具可请求一次性允许。Python 通过 python_run 的 access 数组预声明额外路径：
`{"path":"/absolute/extra","write":false}`。不自动重跑已经因权限失败而可能产生副作用的脚本。
确认过期、拒绝、取消或目标路径变化均失败。通用 Python 不会在每个系统调用弹窗，未事先授权的访问由 OS 拒绝。

Codex / Claude 子进程也使用相同原生目录后端。真实登录凭据在工作目录外时需要单次导入授权；API Key 模式仅传所选 Provider 所需环境变量。实际厂商 CLI 的所有版本和登录方式尚未逐一验证。

## 生命周期与验证

没有独立沙箱 HTTP 服务、镜像、容器清理队列或外部执行后端。进程内 supervisor 管理并发、超时、调用者取消和关闭；执行 future 的 RAII 负责终止进程组和清理暂存目录。

测试覆盖真实 macOS 文件边界与原生 Python Skill 执行，模型和 Codex 协议使用本机替身。Linux 分支需在 Linux 环境验证，不把 macOS 结果等同为 Linux 隔离证明。
