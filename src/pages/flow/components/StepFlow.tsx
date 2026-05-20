import {
  Background,
  Connection,
  Controls,
  Edge,
  Handle,
  MarkerType,
  NodeProps,
  EdgeChange,
  NodeChange,
  Position,
  ReactFlow,
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
} from "@xyflow/react";
import { useCallback, useMemo } from "react";

import { nodeType } from "..";
import { useIntl } from "react-intl";
import { useWorkflowStore } from "../../../models/workflow";

const StepNode = ({ type, data }: NodeProps<WorkflowNode>) => {
  const intl = useIntl();
  return (
    <div>
      <Handle type="target" position={Position.Top} />
      <div style={{ color: "#8c8c8c", fontSize: 12, textAlign: "left" }}>
        {intl.formatMessage(nodeType[type as keyof typeof nodeType])}
      </div>
      <div style={{ fontSize: 14, fontWeight: 500 }}>{data.name}</div>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
};

export default () => {
  const { select, setSelect } = useWorkflowStore();

  const onNodesChange = useCallback(
    (changes: NodeChange<WorkflowNode>[]) => {
      setSelect({
        ...select,
        nodes: applyNodeChanges(changes, select.nodes) as WorkflowNode[],
      });
    },
    [select, setSelect],
  );

  const onEdgesChange = useCallback(
    (changes: EdgeChange<Edge>[]) => {
      setSelect({
        ...select,
        edges: applyEdgeChanges(changes, select.edges),
      });
    },
    [select, setSelect],
  );

  const onConnect = useCallback(
    (params: Connection) =>
      //连线要结束箭头
      setSelect({
        ...select,
        edges: addEdge(
          {
            ...params,
            markerEnd: {
              type: MarkerType.ArrowClosed,
            },
          },
          select.edges,
        ),
      }),
    [select, setSelect],
  );

  const nodeTypes = useMemo(
    () =>
      Object.fromEntries(
        Object.keys(nodeType).map((type) => [type, StepNode]),
      ) as Record<string, typeof StepNode>,
    [],
  );

  return (
    <ReactFlow
      nodes={select.nodes}
      edges={select.edges}
      nodeTypes={nodeTypes}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onConnect={onConnect}
      deleteKeyCode={["Backspace", "Delete"]}
      fitView
      colorMode="light"
      defaultEdgeOptions={{
        markerEnd: {
          type: MarkerType.ArrowClosed,
        },
      }}
    >
      <Controls />
      <Background />
    </ReactFlow>
  );
};
