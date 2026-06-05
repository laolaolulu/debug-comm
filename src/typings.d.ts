import type { ProFormColumnsType } from "@ant-design/pro-components";
import type { Edge, Node } from "@xyflow/react";

declare global {
  type Workflow = {
    // 工作流唯一标识，当前用创建时间戳生成。
    id: string;
    // 工作流名称，会展示在顶部任务选择器里。
    name: string;
    // 工作流描述，会展示在右侧任务信息表单里。
    description?: string;
    // 画布上的节点列表。
    nodes: WorkflowNode[];
    // 画布上的连线列表。
    edges: Edge[];
  };

  type WorkflowStartError = {
    stepId: string;
    stepName: string;
    stepType: string;
    message: string;
  };

  type WorkflowStartResult = {
    started: boolean;
    errors: WorkflowStartError[];
  };

  // 单个 ReactFlow 节点的业务数据，必须和后端 WorkflowNodeData 保持字段语义一致。
  type WorkflowNodeData = {
    // 节点显示名称，对齐后端 WorkflowNodeData.name，右侧表单会直接编辑这个字段。
    name: string;
    // 节点说明，对齐后端 WorkflowNodeData.description，用于节点描述和参数面板展示。
    description?: string;
    // 节点参数表单，对齐后端 WorkflowNodeData.columns，React 前端按 ProForm 配置渲染。
    columns?: ProFormColumnsType[];
    // 保留步骤自己的动态参数或 ReactFlow 扩展数据，避免后端新增字段时前端类型拦住。
    [key: string]: unknown;
  };

  // ReactFlow 节点类型别名，统一给节点 data 补上 WorkflowNodeData 类型。
  type WorkflowNode = Node<WorkflowNodeData>;
}

export {};
