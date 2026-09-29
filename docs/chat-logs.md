# 聊天就是追加日志

一组固定聊天 ID，而不是一组执行轮次：

- 业务聊天使用 group ID。新消息自动接续这个群的聊天，不需要 previous_session_id；该参数仅作为旧调用的兼容校验。
- AdminAgent 每个项目只有一个固定 session（ID 等于项目 UUID），重复创建返回同一个 ID。
- run ID 只标识某一次执行、打断或结果，不再是 Web 的聊天列表项。

## 文件结构

```text
CARBOT_DATA_DIR/
  state.json                     # 配置、策略、凭证与轻量执行状态；不存聊天事件
  chats/
    <project UUID>/
      <hex("admin" 或 "group:<group ID>")>/
        hour-000000497385.jsonl
  state.before-chat-jsonl.json    # 旧格式迁移时保留的原始备份
```

文件名中的数字为 Unix 时间除以 3600，小时边界以 UTC 为准；Web 转成本地时间显示。目录 ID 做十六进制编码，避免群名称或 ID 被解释为文件路径。没有消息的小时不会创建文件。

一条事件一行 JSON，包含递增 seq 和 logged_at 秒级时间。写入使用 append，不反复重写整段历史。文本显示约 40ms 合并一次，管理日志最多约 200ms/32 条批量追加，工具边界及结束会立即刷盘。持久化后台线程只持有待写批次；落盘后释放对应实时缓冲。进程被强杀可能丢失尚未刷盘的尾部，但已刷盘行不会被全量覆盖。

聊天日志是事实记录；执行状态只是索引。最终事件先写入日志，再更新状态。启动恢复不会重新执行任务。发现损坏行或不完整末行时返回明确错误，不静默丢弃或覆盖历史；修复前需备份文件。一个数据目录仍由一个 Carbot 进程独占。

旧 history / management_sessions.events 启动时迁移到日志，先保留原始 state.json 备份，成功后才移除快照中的聊天正文。迁移可重试，旧管理会话合并到项目的 AdminAgent 聊天；缺少时间信息的旧会话按现有元数据稳定排序，不伪造精确的跨会话时序。

## Web 读取接口

```text
GET /v1/repl/{project}/chats/{admin|group ID}/logs
GET /v1/repl/{project}/chats/{admin|group ID}/logs?before=123&limit=300
GET /v1/repl/{project}/chats/{admin|group ID}/logs?after=123&limit=300
GET /v1/repl/{project}/chats/{admin|group ID}/events?after=123
```

logs 返回 events、files（小时文件名和大小）、active_run、has_more。默认最近 300 条，最多 1000 条；before 向前翻页，after 向后读取。SSE 支持续传游标，业务流读取已落盘增量，管理流使用实时通知。所有接口沿用节点管理权限及项目/群校验，不提供任意文件路径访问。

Web 左侧是群列表和该群的小时日志目录；右侧跨小时连续展示聊天，可加载更早的记录。工具执行与结果合并为默认收起的卡片。

日志保留不代表每次全部送给模型：当前管理上下文读取最近最多 10000 条事件，业务群最多 2000 条，仍有 512 KiB 上下文保护。更早的记录留在文件中供分页查看；自动摘要/长期记忆压缩尚未实现。

## 启动权限

```sh
./agent-node start
./agent-node start --admin-token 'your-token'
# 或通过进程环境指定，避免口令出现在命令行参数中
ADMIN_TOKEN='your-token' ./agent-node start
```

默认空口令只在 localhost/回环地址启用。监听局域网或公网必须设置 ADMIN_TOKEN，并在跨网络部署时使用 HTTPS。旧 admin-token 文件保留但不自动读取。
