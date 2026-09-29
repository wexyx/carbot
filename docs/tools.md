# 统一工具与注册

Carbot 内置工具和 Skill 服务工具使用同一个 `Tool` 接口、`ToolDefinition` 描述、
`ToolRegistry` 分发与 `ToolFactory` 构造流程。参考 crabot 的 inventory 工厂收集方式，
不使用全局可变工具实例；每次构造携带当前工作目录、项目 Skill 快照和执行策略。

- Carbot：注册表定义转换成 Chat / Responses / Anthropic 原生工具 schema。
- Codex / Claude / Mock：ToolRuntime 提供 JSON 工具桥接；Mock 本身不产生智能工具决策。

Provider 通过 `handles_tools` 声明是否自行管理工具循环。Carbot 只运行自己的原生工具循环，不再外套 JSON 桥接，避免冲突指令和两层循环重复执行；Skill 目录元数据仍注入原生运行时。
- 桥接只解析调用与驱动循环，不实现工具业务或权限判断。
- list_files、read_file、skill_read、skill_file、python_run、command_run 都是同级 Tool。
- Skill 仍是指令/资源包，不是可绕过授权的宿主插件。

## 属性宏自动注册

### Web 工具管理

「管理 / 项目」侧边栏分别提供管理工具、项目工具入口。项目工具按本地 Agent 隔离：内置注册工具可以启用/禁用，不可删除；外部固定 Shell 命令工具支持新增、编辑、启停、删除。外部工具使用同一 Tool/Factory 契约，不能覆盖内置名称，也不会插值模型参数。所有外部命令仍经过逐次人类确认与原生沙箱。

配置保存在当前实例的文件存储中，使用版本校验防止覆盖并发修改。禁用会同时移除下一轮的模型工具定义和执行入口；已经开始的轮次保留其工具快照。这里只控制 Carbot 注册工具，不控制 Codex/Claude 自带的私有工具。远端 Agent 的工具配置在其所属节点维护。

管理接口（沿用本机访问/管理员认证保护）：`GET/PUT /v1/repl/{project}/tool-config/{management|business}/{agent}`。管理 Agent 标识为 `admin`；PUT 参数为 `{expected_version, policy:{disabled:[内置工具名], external:[{name,description,command,enabled}]}}`。内置工具定义不允许从此接口修改。

Skills 入口同样按管理/项目区分，可创建、编辑文件与启停，内置管理指南只读。`GET/PUT /v1/repl/{project}/skills/{management|business}`；PUT 使用 `{expected_version,definition}`。这些编辑不自动授予脚本执行权限。

### 上传与试运行

项目工具可导入单个定义或定义数组的 JSON：`{"name":"show_status","description":"查看状态","command":"git status --short"}`。导入先预览，保存默认禁用，不能覆盖同名内置工具。这是固定命令适配器，不是上传执行 Rust 插件。

Skill 可上传 UTF-8 文件、文件夹或 `{id,description,files}` JSON 包；必须包含非空 `SKILL.md`，不支持 ZIP。最多 32 个文件，单文件 64 KiB、内容合计 256 KiB。路径穿越和重复文件会被拒绝。导入默认禁用，不继承脚本授权。

详情中的「运行测试」使用真实 ToolRegistry 和当前作用域配置；填入符合工具参数说明的 JSON，例如 list_files 使用 `{"path":"."}`。执行可能修改数据或联网，不是模拟预览。外部命令仍需逐次批准；Python 仍需既有宿主和 Skill 授权，测试不会自动授予权限。离开测试区取消尚未结束的测试；已经完成的副作用不会回滚。

测试接口：`POST /v1/repl/{project}/tool-config/{scope}/{agent}/tests`，参数 `{name,arguments}`；`GET/DELETE /v1/repl/{project}/tool-tests/{id}` 查询或取消。沿用管理员保护，返回任务状态与输出；最多 4 个并行测试、每次最多 240 秒、输出最多 64 KiB。结果保存在有界内存中，重启即清空，不写入聊天历史。

业务工具 `command_run({"command":"..."})` 使用同样的属性宏/工厂注册。在当前 Agent 工作目录中通过原生沙箱运行 `/bin/sh -c`，每次都要求用户确认完整命令（不依赖不可靠的危险命令黑名单）。确认只授权本次执行，不授予目录外访问权限。CLI 权限面板和 Web 均可批准/拒绝；120 秒未批准会失败，取消任务撤销待审批请求。

命令继承宿主沙箱 profile 的网络和超时策略；限制输出、CPU、文件大小及文件描述符，取消/超时终止进程组。不会自动注入模型密钥或宿主环境凭据。不能保证批准后的命令不删除工作区文件或不联网；请核对命令。沙箱不可用时失败，不降级为裸执行。

参考 oxygen_middleware 的 bean_constructor / dynamic_event：扫描 inherent impl 的
new 与 execute，自动生成接口适配器与 inventory 提交代码。工具不再手写 impl Tool
或 register_tool!；名称、说明和参数 schema 都在属性中声明。

```rust
use agent_runtime::tools::{tool, ToolContext, ToolSession};
use serde_json::{Value, json};
use std::sync::Arc;

struct Echo;

#[tool(
    name = "echo",
    description = "Return the input",
    parameters = json!({
        "type": "object",
        "properties": {"message": {"type": "string"}},
        "required": ["message"],
        "additionalProperties": false
    })
)]
impl Echo {
    fn new(_context: Arc<ToolContext>) -> Option<Self> {
        Some(Self)
    }

    async fn execute(
        &self,
        args: &Value,
        _session: &mut ToolSession,
    ) -> Result<Value, String> {
        // schema 用于模型描述；实现仍负责验证参数。
        let message = args["message"].as_str().ok_or("message is required")?;
        if args.as_object().is_none_or(|fields| fields.len() != 1) {
            return Err("unexpected arguments".into());
        }
        Ok(json!({"message": message}))
    }
}
```

最小声明是 `#[tool(name = "echo")]`：description 默认等于 name，parameters 默认
为 object schema。生产工具应声明准确 schema。宏拒绝重复/未知选项、无效名称、
泛型 impl、trait impl 和缺少约定方法的实现；完整参数类型由 Rust 编译器检查。

约定 new(Arc<ToolContext>) -> Option<Self>；返回 None 表示当前项目不启用。
可读取 context.workdir() / context.skills()。execute 是上面签名的异步方法。
name 仅在属性定义一次，不需要维护工具名称列表。

宏在链接时由 inventory 收集构造器，Runtime 初始化时由 ToolFactory 自动创建并注册。
工具模块仍需在 mod.rs 声明并链接进二进制；这不是目录扫描或动态库加载。
agent-runtime 自己内部使用 `runtime = crate`；外部默认路径为 ::agent_runtime，
依赖重命名时可显式指定 `runtime = your_alias`。

## 运行时注册

```rust
let context = ToolContext::new(Some(workdir), catalog, policy)?;
let mut registry = ToolFactory::create(context)?;
// 对于运行时才获得的工具对象，仍可直接注册：
registry.register(tool_instance)?;
registry.unregister("unused_tool");
let runtime = RuntimeFactory::from_config_with_tools(config, registry.clone())?;
```

register 接受任意实现 Tool 的对象，拒绝重复名称，不能静默覆盖权限受控工具。
unregister 影响当前注册表；clone 是注册集合快照，运行中的 Runtime 不会被外部增删影响。
工具实例由 Arc 共享，因此实现者应将每次调用状态放入调用作用域，而非可变全局状态。
默认工厂会向模型提供项目 Skill 目录；自定义 from_config_with_tools 入口只注入工具定义，
使用者需要在任务上下文中提供需要发现的 Skill ID/目录。

动态注册是宿主 Rust API；Web 可维护固定外部命令定义，不支持上传宿主 Rust 代码执行。
宏注册新 Rust 实现需要重新编译；宿主已具备的适配器（例如将来 MCP 客户端）
则可以运行时构造并注册，不需要新增名称分支。

## 权限与兼容

工具名称为 1–64 位 ASCII 字母、数字或下划线，以兼容模型函数协议。
原生调用和 JSON 桥接直接使用相同注册名，不再接受旧点号别名。宏 scope 可为 business（默认）、management 或 shared；工厂先按能力包过滤，再调用构造器。
ToolDefinition 描述参数；工具实现负责反序列化和验证参数，注册表不替自定义 Tool
运行通用 JSON Schema 验证器。内置工具拒绝未知字段。

工具授权在实现内部：文件路径仍受工作目录与敏感路径限制；
python_run 要求本次工具循环已加载该 Skill，并继续验证项目授权、宿主开关、
profile 白名单和进程内沙箱 supervisor。原生调用与 JSON 桥接使用相同实现。
原生工具循环与 JSON 桥接各有独立 ToolSession；如果切换调用入口，需要重新读取 Skill。
新增自定义工具是可信宿主代码，不自动获得沙箱隔离；不得将不可信代码注册为宿主 Tool。

## 验证

`cargo test --workspace --offline` 覆盖宏收集、动态注册/删除、重复拒绝、
快照隔离、Skill 授权与三个模型协议的原生文件/Skill 调用。
`node --test scripts/skills-smoke.test.mjs` 覆盖 CLI 桥接到进程内沙箱 的链路。
沙箱烟测执行真实原生 Python；模型和 CLI 协议使用本机替身。

## 统一能力库与四层规则

定义集中在本节点能力库维护，管理与项目能力保持隔离。新工具/Skill 默认停用，编辑一次影响所有引用位置。Web「项目工具库 / 项目 Skill 库」可切换项目与 Agent 查看最终状态。

启用设置为三态：继承、启用、禁用；优先级为 **项目内 Agent > Agent > 项目 > 全局 > 定义默认值**。Agent 层按本节点 client_id 跨项目匹配。启用不授予 Python、目录外访问或 Shell 确认权限，也不控制供应商 CLI 私有工具。

旧定义不复制、不删除，集中显示并保留原项目/Agent 的默认绑定范围。同名但不同内容的旧定义分别展示；在同一执行范围启用重名定义会报错，不会静默覆盖。旧工具/Skill 管理接口仍可编辑原定义，但 Web 新入口使用统一接口。

- GET/PUT `/v1/repl/{project}/capabilities/{management|business}/{tool|skill}`：列出全节点定义与生效状态；PUT 为 `{id?,expected_version,definition}`，删除为 `{id,expected_version,deleted:true}`，内置不可编辑/删除。
- PUT 同路径 `/bindings?agent=CLIENT_ID`：`{id,layer,enabled,expected_version}`，layer 为 global/project/agent/project_agent，enabled 为 true/false/null（继承）。
- GET 添加 `?agent=CLIENT_ID` 可查看四层来源；未选 Agent 时只展示前两层。
- 所有接口沿用管理员认证；配置持久化，下一轮执行加载，不修改进行中的执行快照。
