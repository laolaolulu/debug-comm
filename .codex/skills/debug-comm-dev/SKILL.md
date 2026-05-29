---
name: debug-comm-dev
description: 在 debug-comm 仓库中进行功能开发、问题修复、小范围重构、Tauri 命令、React/TypeScript 界面、Zustand 状态、react-intl 文案、工作流节点或 Rust/Tokio 通信步骤开发时使用。遵守本项目精简直接的实现风格，避免不必要的抽象。
---

# debug-comm 开发

## 项目形态

- 这是一个 Tauri 2 桌面通信调试工具：前端使用 React 19 + TypeScript + Vite，UI 使用 Ant Design / Pro Components，设计器使用 React Flow，状态使用 Zustand，后端使用 Rust + Tokio。
- 修改应贴近当前功能或 bug，优先沿用现有模块布局，不新建不必要的目录、层级或框架式抽象。
- 代码保持精简、直接、可读。一两行逻辑不要提取函数，除非它被复用、表达明确领域概念，或能明显降低复杂度。
- 不添加宽泛防御分支、通用验证器、服务层或“以后可能会用”的扩展点，除非当前行为确实需要。

## 前端

- UI 放在 `src/pages`，共享工作流状态放在 `src/models`，Tauri Store 持久化通过 `src/appStore.ts`。
- 沿用当前组件风格：函数组件、Ant Design 布局/表单/按钮、Pro Components schema form，小型局部 helper 靠近调用处。
- 添加状态前先看现有 Zustand store 是否能承载。只持久化重启后仍需要的用户可见状态。
- UI 文案使用 `FormattedMessage`、`useIntl` 或 `defineMessages`。新增或修改文案后运行 `pnpm i18n:extract`。
- 设计器和工作台行为要贴合现有 `Workflow`、`WorkflowNode` 与 React Flow 数据结构。

## 后端

- 新增工作流步骤时沿用现有 step 模式：
  - 用 `serde::{Serialize, Deserialize}` 定义 step data；
  - 持有 `BaseStepContext`；
  - 暴露 `new(node: &WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String>`；
  - 实现 `BaseStep` 和 `StepManifestProvider`；
  - 在 `Workflow::available_steps()` 注册 manifest；
  - 在 `Workflow::instantiate_step()` 实例化步骤。
- 使用 step 模块里已有的 Tokio/Tauri async runtime 写法。步骤持有后台任务时，用 `Drop` 做简单关闭。
- Tauri/workflow 边界返回 `Result<_, String>`，错误信息包含 step id 或操作名称。
- 逻辑不平凡或容易回归时，把 Rust 单元测试放在被修改模块内。

## 验证

- 优先运行最窄但有效的检查：前端/类型改动跑 `pnpm build`，文案改动跑 `pnpm i18n:extract`，Rust 逻辑改动在 `src-tauri` 下跑 `cargo test`。
- 不运行会重写已跟踪文件的格式化或生成命令，除非任务本身需要这些输出。
- 收尾时只说明改了什么行为，以及实际跑了哪些检查。
