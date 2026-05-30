# debug-comm

一个基于 **Tauri 2 + React + TypeScript + Rust** 的桌面通信调试工具。

debug-comm 以可视化工作流的方式组织通信链路，支持串口、TCP 客户端、TCP 服务端、发送窗口、接收窗口和 JS 自动化脚本等节点。你可以在设计器里拖拽节点、连接数据流，然后在工作台启动任务、发送数据、查看接收日志。

## 功能特性

- 可视化工作流设计：基于 React Flow 进行节点拖拽、连接和参数配置。
- 串口通信：支持串口号、波特率、数据位、停止位、校验位、流控和结束符配置。
- TCP 客户端：主动连接远端 TCP 服务，支持收发数据。
- TCP 服务端：监听本地端口，接收客户端连接并广播下行数据。
- 发送数据窗口：支持 UTF-8 和 HEX 输入模式，HEX 模式支持大小写、空格和分隔符。
- 接收数据窗口：以日志列表展示接收数据，支持 UTF-8 / HEX 切换、本地持久化和历史加载。
- JS 自动化脚本：使用 boa_engine (Rust 嵌入式 JS 引擎) 处理上下行消息，支持自定义 `read_up`/`read_down` 回调和 `write_up`/`write_down` 转发函数。
- 任务运行管理：启动、停止、查询当前运行任务，停止时释放后台任务和 socket 资源。
- 本地持久化：工作流配置、语言设置和接收日志保存在本地应用数据目录。
- 自动更新：从 GitHub Releases 检查、下载并安装新版本。
- 中英文界面：基于 react-intl 的国际化支持。

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri 2 |
| 前端 | React 19 + TypeScript 5.8 + Vite 7 |
| UI | Ant Design 6 + @ant-design/pro-components |
| 流程图 | @xyflow/react (ReactFlow) |
| 状态管理 | Zustand |
| 消息持久化 | Dexie (IndexedDB) |
| 国际化 | react-intl (中/英文) |
| Rust 后端 | Tokio + serialport + boa_engine |
| 本地能力 | Tauri Store、Dialog、FS、Opener 插件 |
| 自动更新 | Tauri Updater、Process 插件 |

## 基础通信测试演示


 <video src="wwwroot/assets/communication-demo.mp4" controls  width="100%" preload="metadata" poster="wwwroot/assets/poster.jpg"></video>

## 节点类型

| 节点         | 说明                                                                     |
| ------------ | ------------------------------------------------------------------------ |
| 发送数据窗口 | 人工输入数据，并向下游通信节点发送                                       |
| 接收数据窗口 | 展示相邻通信节点返回的数据，并持久化为本地日志                           |
| 串口通信     | 打开串口，接收下行数据写入串口，并将串口返回数据发布出去                 |
| TCP 客户端   | 主动连接远端 TCP 服务，支持读写                                          |
| TCP 服务端   | 监听本地端口，接收客户端数据并支持广播写回                               |
| JS 自动化脚本 | 使用 JavaScript 处理上下行消息，支持 `read_up`/`read_down` 回调和 `write_up`/`write_down` 转发 |

## 项目结构

```text
.
├── src/                           # React 前端
│   ├── main.tsx                   # React 入口
│   ├── App.tsx                    # 根组件（菜单 + 工作台/设计器切换）
│   ├── appStore.ts                # Tauri Store 封装（持久化读写）
│   ├── models/
│   │   ├── workflow.ts            # 工作流 Zustand store（CRUD + 持久化）
│   │   ├── workrun.ts             # 运行中工作流 ID 管理
│   │   ├── msgstore.ts            # 消息 store（IndexedDB + Tauri 事件）
│   │   ├── activeTab.ts           # 当前 Tab 状态
│   │   └── locale.ts              # 语言切换
│   ├── pages/
│   │   ├── flow/                  # 设计器页面
│   │   │   ├── index.tsx          # 三栏布局：StepList | StepFlow | StepPar
│   │   │   └── components/
│   │   │       ├── Header.tsx     # 顶部栏（新建/保存/导入/导出）
│   │   │       ├── StepList.tsx   # 左侧：可拖拽的步骤节点列表
│   │   │       ├── StepFlow.tsx   # 中间：ReactFlow 画布
│   │   │       └── StepPar.tsx    # 右侧：节点参数编辑表单
│   │   ├── work/                  # 工作台页面
│   │   │   ├── index.tsx          # 上下分栏：DisOutput | DisInput
│   │   │   └── components/
│   │   │       ├── Header.tsx     # 选择工作流、启动/停止
│   │   │       ├── DisOutput.tsx  # 接收数据展示窗口
│   │   │       └── DisInput.tsx   # 发送数据输入窗口
│   │   ├── SelectWork.tsx         # 工作流选择下拉组件
│   │   └── RightContent.tsx       # 右上角内容（语言切换等）
│   └── locales/                   # 国际化资源（zh-CN / en-US）
│
├── src-tauri/                     # Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json            # Tauri 配置
│   └── src/
│       ├── main.rs                # Tauri 入口
│       ├── lib.rs                 # Tauri commands 注册
│       └── step/
│           ├── mod.rs             # 模块导出
│           ├── model.rs           # 核心数据模型（Workflow/Node/Edge/StepMsg）
│           ├── workflow.rs         # Workflow 运行引擎（拓扑排序、消息路由）
│           ├── basestep.rs        # BaseStep trait + BaseStepContext
│           ├── disinputstep.rs    # 发送数据窗口步骤
│           ├── disoutputstep.rs   # 接收数据窗口步骤
│           ├── serialportstep.rs  # 串口通信步骤
│           ├── tcpclientstep.rs   # TCP 客户端步骤
│           ├── tcpserverstep.rs   # TCP 服务端步骤
│           └── javascriptstep.rs  # JS 自动化脚本步骤（boa_engine）
│
├── public/                        # 静态资源
├── scripts/                       # 工具脚本
└── .codex/skills/                 # Codex skill 配置
```

## 开发计划

- [x] 基础通信：TCP、串口
- [x] JavaScript 脚本 通信报文模拟
- [ ] Lua 脚本 通信报文模拟
- [ ] 通信报文模拟 脚本编写 `AI` 能力 'Pro'
- [ ] Modbus 通信调试
- [ ] MQTT 协议调试
- [ ] 虚拟串口 `Pro`
- [ ] 串口监控，不占用当前串口
- [ ] 任务配置文件云端同步功能，换个环境一键同步 `Pro`
- [ ] C# (.NET 8) Runtime 开发

## 许可证

本项目基于 [MIT License](./LICENSE) 开源。
