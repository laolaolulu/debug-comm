# debug-comm

一个基于 **Tauri 2 + React + TypeScript + Rust** 的桌面通信调试工具。

debug-comm 以可视化工作流的方式组织通信链路，支持串口、TCP 客户端、TCP 服务端、发送窗口和接收窗口等节点。你可以在设计器里拖拽节点、连接数据流，然后在工作台启动任务、发送数据、查看接收日志。

## 功能特性

- 可视化工作流设计：基于 React Flow 进行节点拖拽、连接和参数配置。
- 串口通信：支持串口号、波特率、数据位、停止位、校验位、流控和结束符配置。
- TCP 客户端：主动连接远端 TCP 服务，支持收发数据。
- TCP 服务端：监听本地端口，接收客户端连接并广播下行数据。
- 发送数据窗口：支持 UTF-8 和 HEX 输入模式，HEX 模式支持大小写、空格和分隔符。
- 接收数据窗口：以日志列表展示接收数据，支持 UTF-8 / HEX 切换、本地持久化和历史加载。
- 任务运行管理：启动、停止、查询当前运行任务，停止时释放后台任务和 socket 资源。
- 本地持久化：工作流配置、语言设置和接收日志保存在本地应用数据目录。
- 中英文界面：基于 react-intl 的国际化支持。

## 技术栈

- 桌面框架：Tauri 2
- 前端：React 19、TypeScript、Vite
- UI：Ant Design、Ant Design Pro Components
- 工作流画布：@xyflow/react
- 状态管理：Zustand
- 后端：Rust、Tokio、serialport
- 本地能力：Tauri Store、Dialog、FS、Opener 插件

## 快速开始

### 环境要求

请先安装：

- Node.js
- pnpm
- Rust stable
- Tauri 2 所需系统依赖

Windows 环境通常还需要安装 Microsoft C++ Build Tools 和 WebView2 Runtime。

### 安装依赖

```bash
pnpm install
```

### 启动开发模式

```bash
pnpm tauri dev
```

### 前端构建

```bash
pnpm build
```

### Rust 测试

```bash
cd src-tauri
cargo test
```

### 打包安装程序

```bash
pnpm tauri build
```

当前 Tauri 配置默认打包 Windows MSI 安装包。

## 使用说明

1. 打开应用后进入工作台或设计器。
2. 在设计器中创建通信工作流：
   - 添加发送数据窗口。
   - 添加串口 / TCP 客户端 / TCP 服务端节点。
   - 添加接收数据窗口。
   - 按数据流方向连接节点。
3. 配置每个通信节点的参数。
4. 保存工作流。
5. 回到工作台，点击启动。
6. 在发送窗口输入 UTF-8 或 HEX 数据并发送。
7. 在接收窗口查看接收日志。
8. 点击停止释放任务、串口和 socket 资源。

## 节点类型

| 节点 | 说明 |
| --- | --- |
| 发送数据窗口 | 人工输入数据，并向下游通信节点发送 |
| 接收数据窗口 | 展示相邻通信节点返回的数据，并持久化为本地日志 |
| 串口通信 | 打开串口，接收下行数据写入串口，并将串口返回数据发布出去 |
| TCP 客户端 | 主动连接远端 TCP 服务，支持读写 |
| TCP 服务端 | 监听本地端口，接收客户端数据并支持广播写回 |

## HEX 输入规则

发送窗口切换到 HEX 模式后，只允许输入：

- `0-9`
- `a-f`
- `A-F`
- 空白字符
- 常用分隔符：`, ; : - _`

以下格式都可以被解析：

```text
a1b2C3
A1 B2 C3
A2-C1
A1,B2:C3
```

发送前会移除分隔符并按两个字符解析为一个字节。

## 接收日志

接收窗口会把收到的数据写入本地日志文件：

- 默认加载最近 100 条。
- 最新日志显示在底部。
- 向上滚动可以加载更早日志。
- 清空按钮会同时清空界面和本地持久化日志。
- 日志按任务 ID 和接收节点 ID 分文件保存。

## 项目结构

```text
.
├── src/                    # React 前端
│   ├── pages/              # 工作台、设计器和布局页面
│   ├── models/             # Zustand 状态管理
│   ├── locales/            # 国际化文案
│   └── appStore.ts         # Tauri Store 封装
├── src-tauri/              # Tauri / Rust 后端
│   ├── src/
│   │   ├── step/           # 工作流和各通信步骤实现
│   │   ├── receive_log.rs  # 接收日志持久化
│   │   └── lib.rs          # Tauri 命令入口
│   ├── capabilities/       # Tauri 权限配置
│   └── tauri.conf.json     # Tauri 应用配置
├── public/                 # 静态资源
├── scripts/                # 工具脚本
└── package.json
```

## 常用命令

```bash
# 启动前端 Vite
pnpm dev

# 启动 Tauri 开发模式
pnpm tauri dev

# 构建前端
pnpm build

# 预览前端构建产物
pnpm preview

# 提取国际化文案
pnpm i18n:extract

# 运行 Rust 测试
cd src-tauri && cargo test
```

## 开发备注

- 前端发送数据通过 `publish_step_message` 命令进入后端工作流。
- 后端步骤之间通过工作流内部广播通道传递 `Up` / `Down` 消息。
- 接收窗口监听相邻通信节点的数据并生成 `ReceiveLogRecord`。
- 停止任务时会显式关闭步骤，释放后台任务、串口和 socket 资源。

## 许可证

本项目基于 [MIT License](./LICENSE) 开源。
