import { invoke } from "@tauri-apps/api/core";
import { useDraggable } from "@neodrag/react";
import { useReactFlow, XYPosition } from "@xyflow/react";
import type { ProFormColumnsType } from "@ant-design/pro-components";
import {
  type RefObject,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { Divider, Flex, Typography } from "antd";
import { FormattedMessage, useIntl } from "react-intl";
import { nodeType } from "..";
import { HolderOutlined } from "@ant-design/icons";

interface StepManifest {
  type: keyof typeof nodeType;
  name: string;
  description: string;
  default_data: ProFormColumnsType[];
}

interface DraggableNodeProps {
  step: StepManifest;
  onDrop: (step: StepManifest, position: XYPosition) => void;
}

const DraggableNode = ({ step, onDrop }: DraggableNodeProps) => {
  const draggableRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<XYPosition>({ x: 0, y: 0 });
  const intl = useIntl();
  useDraggable(draggableRef as RefObject<HTMLDivElement>, {
    position: position,
    onDrag: ({ offsetX, offsetY }) => {
      setPosition({
        x: offsetX,
        y: offsetY,
      });
    },
    onDragEnd: ({ event }) => {
      const rect = draggableRef.current?.getBoundingClientRect();
      setPosition({ x: 0, y: 0 });
      onDrop(step, {
        x: rect?.left ?? event.clientX,
        y: rect?.top ?? event.clientY,
      });
    },
  });
  return (
    <div ref={draggableRef} className="listnode">
      <span className="listnode-title">
        <span>{intl.formatMessage(nodeType[step.type])}</span>
      </span>
      <HolderOutlined style={{ color: "#666" }} />
    </div>
  );
};

export default () => {
  const { setNodes, screenToFlowPosition } = useReactFlow();
  const [steps, setSteps] = useState<StepManifest[]>([]);

  useEffect(() => {
    invoke<StepManifest[]>("get_step_manifests")
      .then(setSteps)
      .catch(() => setSteps([]));
  }, []);

  const handleNodeDrop = useCallback(
    (step: StepManifest, screenPosition: XYPosition) => {
      const flow = document.querySelector(".react-flow");
      const flowRect = flow?.getBoundingClientRect();
      const isInFlow =
        flowRect &&
        screenPosition.x >= flowRect.left &&
        screenPosition.x <= flowRect.right &&
        screenPosition.y >= flowRect.top &&
        screenPosition.y <= flowRect.bottom;

      // Create a new node and add it to the flow
      if (isInFlow) {
        const position = screenToFlowPosition(screenPosition);

        const newNode: WorkflowNode = {
          id: String(Date.now()),
          type: step.type,
          position,
          data: {
            name: step.name,
            description: step.description,
            columns: step.default_data,
          },
        };

        setNodes((nds) => nds.concat(newNode));
      }
    },
    [setNodes, screenToFlowPosition],
  );

  return (
    <Flex vertical style={{}}>
      <Flex justify="space-between" style={{ margin: "15px 15px 5px 15px" }}>
        <Typography.Text strong>
          <FormattedMessage id="step.list.title" defaultMessage="工作流节点" />
        </Typography.Text>
        <Typography.Text type="secondary">
          <FormattedMessage
            id="step.list.count"
            defaultMessage="{count} 个类型"
            values={{ count: steps.length }}
          />
        </Typography.Text>
      </Flex>
      <Divider size="small" />
      <Flex
        gap={10}
        vertical
        style={{
          padding: "5px 15px 10px 10px",
          overflowX: "hidden",
          overflowY: "auto",
          height: "calc(100vh - 160px)",
          scrollbarWidth: "thin",
        }}
      >
        {steps.map((step) => (
          <DraggableNode key={step.type} step={step} onDrop={handleNodeDrop} />
        ))}
      </Flex>
    </Flex>
  );
};
