import { ClearOutlined } from "@ant-design/icons";
import { listen } from "@tauri-apps/api/event";
import {
  Button,
  Empty,
  Flex,
  Input,
  Segmented,
  Space,
  Splitter,
  Typography,
} from "antd";
import { useEffect, useMemo, useState } from "react";
import { FormattedMessage, useIntl } from "react-intl";
import { useWorkflowStore } from "../../../models/workflow";

type OutputMode = "utf-8" | "hex";
type WorkflowStepMessage = {
  workflow_id: string;
  step_id: string;
  source_step_id: string;
  msg: unknown;
};

const bytesToHex = (bytes: number[]) =>
  bytes
    .map((byte) => byte.toString(16).padStart(2, "0").toUpperCase())
    .join(" ");

const formatMessage = (msg: unknown, mode: OutputMode) => {
  if (Array.isArray(msg) && msg.every((item) => typeof item === "number")) {
    return mode === "hex"
      ? bytesToHex(msg)
      : new TextDecoder().decode(new Uint8Array(msg));
  }

  if (typeof msg === "string") {
    return mode === "hex"
      ? bytesToHex([...new TextEncoder().encode(msg)])
      : msg;
  }

  return JSON.stringify(msg);
};

function OutputPanel({
  node,
  messages,
  onClear,
}: {
  node: WorkflowNode;
  messages: WorkflowStepMessage[];
  onClear: () => void;
}) {
  const [mode, setMode] = useState<OutputMode>("utf-8");
  const intl = useIntl();
  const value = useMemo(
    () =>
      messages
        .map((item) => formatMessage(item.msg, mode))
        .filter(Boolean)
        .join("\n"),
    [messages, mode],
  );
  const placeholder =
    mode === "hex"
      ? intl.formatMessage({
          id: "work.output.placeholder.hex",
          defaultMessage: "等待接收 HEX 数据",
        })
      : intl.formatMessage({
          id: "work.output.placeholder.text",
          defaultMessage: "等待接收数据",
        });

  return (
    <div
      style={{
        height: "calc(100% - 10px)",
        display: "flex",
        flexDirection: "column",
        margin: "0 10px 10px 10px",
        padding: "10px 12px",
        background: "#fff",
        border: "1px solid #d9d9d9",
        borderRadius: 15,
        boxShadow: "0 1px 4px rgba(15, 23, 42, 0.04)",
      }}
    >
      <Flex justify="space-between" align="flex-start">
        <Typography.Text
          ellipsis
          style={{ color: "#1677ff", fontSize: 13, maxWidth: 160 }}
        >
          {node.data.name}
        </Typography.Text>
        <Space>
          <Segmented
            value={mode}
            options={["utf-8", "hex"]}
            onChange={(next) => setMode(next as OutputMode)}
          />
          <Button icon={<ClearOutlined />} disabled={!value} onClick={onClear}>
            <FormattedMessage id="work.output.clear" defaultMessage="清空" />
          </Button>
        </Space>
      </Flex>
      <Input.TextArea
        readOnly
        variant="borderless"
        value={value}
        placeholder={placeholder}
        style={{ flex: 1, padding: 0, resize: "none" }}
      />
    </div>
  );
}

export default () => {
  const select = useWorkflowStore((state) => state.select);
  const intl = useIntl();
  const [messages, setMessages] = useState<
    Record<string, WorkflowStepMessage[]>
  >({});
  const workflowId = select?.id;
  const nodes = select?.nodes.filter((f) => f.type === "DisOutputStep") ?? [];

  useEffect(() => {
    setMessages({});
  }, [workflowId]);

  useEffect(() => {
    const unlisten = listen<WorkflowStepMessage>(
      "workflow-step-message",
      ({ payload }) => {
        if (payload.workflow_id !== workflowId) {
          return;
        }
        setMessages((current) => ({
          ...current,
          [payload.step_id]: [...(current[payload.step_id] ?? []), payload],
        }));
      },
    );

    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, [workflowId]);

  if (nodes.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={intl.formatMessage({
          id: "work.output.noNodes",
          defaultMessage: "暂无接收节点",
        })}
      />
    );
  }

  return (
    <Splitter style={{ height: "100%" }}>
      {nodes.map((node) => (
        <Splitter.Panel key={node.id}>
          <OutputPanel
            node={node}
            messages={messages[node.id] ?? []}
            onClear={() =>
              setMessages((current) => ({ ...current, [node.id]: [] }))
            }
          />
        </Splitter.Panel>
      ))}
    </Splitter>
  );
};
