# 工作流设计说明

`src-tauri/src/step` 目录负责后端工作流运行逻辑。前端工作流设计器传入 JSON 后，后端将 JSON 解析成 `WorkflowDefinition`，创建 `Workflow` 实例，再根据节点类型实例化具体步骤。步骤之间不直接持有彼此，而是通过 `Workflow::publish` 按连线方向分发消息。

当前工作流的核心设计可以概括为：

- `WorkflowDefinition` 保存工作流静态结构：节点、连线、名称、描述。
- `Workflow` 保存工作流运行实例：步骤实例集合、连线关系、全局注册关系。
- `BaseStep` 是所有步骤的统一抽象，每个具体步骤负责自己的资源初始化、消息处理和后台任务。
- `MsgType::Down` 表示数据沿 `edge.source -> edge.target` 向下游流动。
- `MsgType::Up` 表示数据从下游节点返回上游节点。

## 对外入口

工作流对外入口在 `src-tauri/src/lib.rs` 中，Tauri command 会调用 `Workflow` 的能力：

- `start_workflow(json)`：解析 JSON，创建 `Workflow`，调用 `run` 启动工作流，返回 workflow id。
- `stop_workflow(id)`：按 id 查找工作流，如果存在则从全局注册表移除。
- `get_workflow_ids()`：返回当前全局注册表中仍存活的工作流 id。
- `get_step_manifests()`：返回后端当前支持的步骤清单，用于前端展示可拖拽节点。
- `get_serial_ports()`：返回当前系统可用串口名称。

## 工作流执行逻辑

1. 前端传入工作流 JSON。
2. `start_workflow` 将 JSON 反序列化为 `WorkflowDefinition`。
3. `Workflow::new` 创建工作流实例：
   - 保存前端传入的工作流定义。
   - 将实例注册到全局 `WORKFLOW_INSTANCES` 中，方便查询正在执行的任务，也方便停止任务时释放实例。
4. 调用 `Workflow::run` 后开始装配步骤：
   - `sort_nodes` 根据 `edges` 做拓扑排序，上游节点优先实例化。
   - 每个节点通过 `instantiate_step` 按 `node.type` 创建对应步骤。
   - 已实现的类型会创建具体步骤，例如 `serialportstep`、`disoutputstep`。
   - 未实现或未知类型会直接返回错误，避免工作流带着无行为节点继续运行。
   - 创建完成后的步骤实例保存到 `Workflow.steps`，由工作流持有生命周期。
5. 具体步骤在构造函数中启动自己的运行逻辑：
   - 例如 `SerialPortStep` 会打开串口，并启动读串口任务。
   - 上级节点发来下行消息时，`Workflow` 调用下级步骤的 `read_up`。
   - 串口读任务从设备读到数据后，通过 `write_up` 向上级节点发布返回消息。
6. `Workflow::publish` 将任意可序列化数据转成 `serde_json::Value`，包装成 `StepMsg<Value>` 后按 `edges` 分发：
   - `Down` 分发给下级步骤并触发 `read_up`。
   - `Up` 分发给上级步骤并触发 `read_down`。
7. 工作流或步骤释放时：
   - `Workflow::drop` 会从全局注册表移除当前工作流。
   - 各步骤的 `Drop` 会关闭运行标记并中止后台任务。

## 节点关系与消息方向

`edges` 的方向表示前端画布上的主数据流方向：

```text
edge.source -> edge.target
上游节点       下游节点
```

`MsgType` 定义两种消息方向：

- `MsgType::Down`：下行消息，从上游节点发送到下游节点，触发下游步骤 `read_up`。
- `MsgType::Up`：上行消息，从下游节点返回上游节点，触发上游步骤 `read_down`。

示例：

```text
disinputstep -> serialportstep -> disoutputstep
```

- `disinputstep` 发布 `Down` 后，`serialportstep.read_up` 会收到消息并写入串口。
- `serialportstep` 读到串口数据后调用 `write_up`，上游节点的 `read_down` 会收到返回消息。
- 如果希望 `disoutputstep` 显示串口返回数据，应让接收窗口作为通信节点的上级。

## 数据模型设计

### `WorkflowDefinition`

完整工作流定义，对齐前端设计器保存的数据：

- `id`：工作流唯一标识。
- `name`：工作流名称。
- `description`：工作流说明。
- `nodes`：节点列表。
- `edges`：连线列表。

### `WorkflowNode`

单个节点定义，对齐 ReactFlow node：

- `id`：节点唯一标识。
- `type`：节点类型，例如 `serialportstep`。
- `position`：节点坐标。
- `data`：节点业务数据和参数表单。
- `selected`：前端选中状态。
- `extra`：保留未显式声明的 ReactFlow 扩展字段。

### `WorkflowNodeData`

节点业务数据，当前设计倾向于包含：

- `name`：节点显示名称，前端可修改。
- `description`：节点说明，前端可修改。
- `columns`：参数表单定义，对齐前端 `ProFormColumnsType[]`。
- `extra`：保留其他扩展字段。

`WorkflowNodeData::parse<T>` 负责将通用节点 data 转换成具体步骤参数：

- 从 `columns` 中递归读取 `dataIndex` 和 `initialValue`。
- 合并节点 data 中的扩展字段。
- 最后反序列化为具体步骤的 data 结构，例如 `SerialPortStepData`。

## 文件说明

### `workflow.rs`

工作流运行核心。

核心职责：

- 创建和保存工作流定义。
- 管理工作流实例注册表。
- 按工作流节点和连线实例化步骤。
- 提供步骤间消息发布和按连线分发能力。
- 维护步骤实例生命周期。

主要成员：

- `definition`：前端传入的工作流定义。
- `steps`：当前运行中的步骤实例集合。
- `WORKFLOW_INSTANCES`：全局工作流注册表，按工作流 id 保存 `Arc<Workflow>`。

主要方法：

- `available_steps`：返回当前支持的步骤清单，目前包含 `SerialPortStep` 和 `DisOutputStep`。
- `from_json`：从 JSON 字符串创建工作流。
- `run`：排序并实例化所有节点。
- `publish`：发布步骤消息。
- `get`、`list`、`list_ids`、`remove`：全局工作流实例管理。
- `sort_nodes`：根据连线做拓扑排序。

设计细节：

- `publish` 根据 `MsgType` 和 `edges` 找到目标步骤并直接调用 `read_up` 或 `read_down`。
- 拓扑排序只影响步骤实例化顺序，不代表设备数据会自动产生。消息是否继续传递由具体步骤的业务逻辑决定。
- 如果图中存在环或异常节点，`sort_nodes` 会按原始节点顺序补齐剩余节点，避免节点丢失。

### `basestep.rs`

定义所有步骤的基础抽象。

核心结构和 trait：

- `BaseStepContext`：步骤基础上下文，保存当前节点 id 和所属工作流的弱引用。
- `BaseStep`：所有步骤都要实现的公共 trait，提供 `read_up` 和 `read_down` 消息回调。
- `StepManifestProvider`：步骤元数据提供者，用于导出前端可创建的步骤类型和默认配置。

设计要点：

- 具体步骤通过组合 `BaseStepContext` 复用基础字段。
- `BaseStepContext` 持有 `Weak<Workflow>`，避免步骤和工作流之间形成强引用循环。
- `write_down` 用于向下级发布消息，`write_up` 用于向上级发布消息。
- 具体步骤应在 `Drop` 中释放自己创建的后台任务、连接、文件句柄或设备资源。

### `model.rs`

定义工作流运行所需的数据模型，主要对齐前端 ReactFlow 和工作流设计器的数据结构。

核心结构：

- `MsgType`：消息方向枚举，包含 `Up` 和 `Down`。
- `StepMsg<T>`：步骤之间传递的消息结构，包含来源步骤 id、消息方向和消息体。
- `WorkflowDefinition`：完整工作流定义。
- `WorkflowNode`：工作流节点结构。
- `WorkflowEdge`：工作流连线结构。
- `WorkflowNodeData`：节点业务数据和参数表单定义。
- `StepManifest`：步骤清单项，用于告诉前端当前后端支持哪些步骤，以及创建节点时的默认 data。

### `mod.rs`

step 模块入口文件，声明并导出当前目录下的各个子模块：

- `basestep`
- `disinputstep`
- `disoutputstep`
- `model`
- `serialportstep`
- `tcpclientstep`
- `tcpserverstep`
- `workflow`

## 节点步骤

### `serialportstep.rs`

串口步骤实现。

当前已实现能力：

- 打开指定串口。
- 通过 `read_up` 读取上游节点下发的数据。
- 将接收到的数据写入串口。
- 阻塞读取串口返回数据。
- 将串口返回数据发布为上行消息。
- 步骤销毁时中止读任务。

当前代码中的节点参数：

- `name`：节点显示名称。
- `description`：节点说明。
- `end_flag`：结束符，16 进制字符串，例如 `0A0D`。设置为空或 `null` 时不检测结束符。
- `port_name`：串口号，例如 `COM1`。
- `baud_rate`：波特率。

- `data_bits`：数据位。
- `stop_bits`：停止位。
- `parity`：校验位。
- `flow_control`：控制流或握手方式。

执行逻辑：

1. `SerialPortStep::new` 解析节点 data 为 `SerialPortStepData`。
2. 使用 `serialport::new(port_name, baud_rate)` 打开串口。
3. clone 串口句柄，分别用于读和写。
4. `read_up` 收到下行消息时，将 `StepMsg.msg` 转换为 `Vec<u8>`，写入串口并 `flush`。
5. 启动读任务：
   - 使用 `spawn_blocking` 执行阻塞串口读取。
   - 读到字节后调用 `write_up`，消息体为 `byte[]`。
6. `Drop` 时设置 `running = false`，并 abort 读任务。

### `tcpclientstep.rs`

TCP 客户端步骤。

当前已实现能力：

- 作为 TCP client 主动连接远端服务。
- 通过 `read_up` 读取上游下行消息，将消息体转成字节后写入 TCP 连接。
- 从 TCP 连接读取返回数据后调用 `write_up`。
- 支持连接超时、最大读取字节数和结束符拆包。

当前节点参数：

- `name`：节点显示名称。
- `description`：节点说明。
- `end_flag`：结束符，16 进制字符串，例如 `0A0D`。设置为空或 `null` 时不检测结束符。
- `host`：远端地址，例如 `127.0.0.1`。
- `port`：远端端口。
- `connect_timeout_ms`：连接超时时间。
- `max_read_bytes`：单次最大读取字节数。

建议执行逻辑：

1. 创建步骤时解析参数并建立 TCP 连接。
2. `read_up` 收到下行消息后写入 socket。
3. 启动读任务，读取 socket 数据，按结束符或读取长度拆包。
4. 读到完整数据后调用 `write_up`。
5. 连接异常时当前任务会退出，后续可继续扩展自动重连。
6. `Drop` 时停止后台任务并关闭连接。

### `tcpserverstep.rs`

TCP 服务端步骤。

当前已实现能力：

- 在本地监听指定地址和端口。
- 接收一个或多个客户端连接。
- 将客户端上行数据发布到工作流。
- 通过 `read_up` 读取工作流下行消息，并广播写回所有客户端。
- 支持最大读取字节数和结束符拆包。

当前节点参数：

- `name`：节点显示名称。
- `description`：节点说明。
- `bind_addr`：监听地址，例如 `0.0.0.0` 或 `127.0.0.1`。
- `port`：监听端口。
- `end_flag`：可选结束符，用于处理粘包和拆包。
- `max_read_bytes`：单次最大读取字节数。

建议执行逻辑：

1. 创建步骤时绑定本地地址并开始监听。
2. 启动 accept 任务，接收客户端连接并登记连接状态。
3. 为每个连接启动读任务，读到客户端数据后调用 `write_up`。
4. `read_up` 收到下行消息后广播写回所有客户端。
5. 客户端断开时清理连接状态。
6. `Drop` 时停止监听、关闭连接并中止后台任务。

### `disoutputstep.rs`

接收数据窗口步骤。

- `name`：节点显示名称。
- `description`：节点说明。

当前已实现内容：

- 创建时解析节点 data。
- 实现 `read_down`，接收下级返回消息并通过 Tauri 事件推送给前端。

### `disinputstep.rs`

- `name`：节点显示名称。
- `description`：节点说明。

当前已实现内容：

- 创建时解析节点 data。
- 只作为发送窗口节点占位和 manifest 提供者。
- 消息发布由外部按 step id 完成。
- 已接入 `Workflow::available_steps` 和 `Workflow::instantiate_step`。

## 扩展一个新步骤的建议流程

1. 新建步骤文件，例如 `xxxstep.rs`。
2. 定义该步骤的 data 结构，并实现 `Serialize`、`Deserialize`。
3. 定义步骤结构体，组合 `BaseStepContext`。
4. 实现 `new(node, workflow)`：
   - 创建 `BaseStepContext`。
   - 调用 `node.data.parse::<XxxStepData>()` 解析参数。
   - 初始化连接、设备、缓存等资源。
   - 按需启动设备或网络读取任务。
5. 实现 `BaseStep`，按方向重写 `read_up` 或 `read_down`。
6. 实现 `StepManifestProvider`，返回步骤类型、名称、说明和默认 data。
7. 在 `mod.rs` 中导出模块。
8. 在 `Workflow::available_steps` 中加入 manifest。
9. 在 `Workflow::instantiate_step` 中按 `node.type` 创建新步骤。
10. 在 `Drop` 中释放资源，停止任务。

## 后续优化建议

- 统一节点显示字段：当前使用 `name/description`。
- 统一消息体格式：底层通信类步骤只接受 `byte[]`。
- 完善拆包策略：串口、TCP client、TCP server 已支持结束符，后续可继续补固定长度、超时归包等策略。
- 完善状态上报：当前连接失败、读写失败主要表现为后台任务退出，后续可增加步骤状态和错误事件。

## 当前状态小结

当前 `step` 模块已经具备工作流实例管理、节点装配、上下游消息过滤、串口读写、TCP client、TCP server、输入窗口占位和输出窗口缓存能力。`DisInputStep` 的前端触发命令、`DisOutputStep` 的前端事件推送、通信步骤的状态上报和错误通道仍可继续完善。
