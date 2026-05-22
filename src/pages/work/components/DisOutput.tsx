import { ClearOutlined } from "@ant-design/icons";
import {
  Button,
  Empty,
  Flex,
  Segmented,
  Space,
  Splitter,
  Typography,
} from "antd";
import dayjs from "dayjs";
import { useEffect, useMemo, useRef, useState } from "react";
import { FormattedMessage } from "react-intl";
import { useMsgStore } from "../../../models/msgstore";
import { useWorkflowStore } from "../../../models/workflow";

const textDecoder = new TextDecoder();

const bytesToHex = (bytes: Uint8Array) =>
  Array.from(bytes, (byte) =>
    byte.toString(16).padStart(2, "0").toUpperCase(),
  ).join(" ");

const getRelatedSendStepIds = (workflow: Workflow, outputStepId: string) => {
  const lowerStepIds = new Set(
    workflow.edges
      .filter((edge) => edge.source === outputStepId)
      .map((edge) => edge.target),
  );
  const nodeById = new Map(workflow.nodes.map((node) => [node.id, node]));

  return [
    ...new Set(
      workflow.edges
        .filter((edge) => lowerStepIds.has(edge.target))
        .map((edge) => edge.source)
        .filter((stepId) => nodeById.get(stepId)?.type === "DisInputStep"),
    ),
  ];
};

const OutputPanel = ({
  node,
  workflow,
}: {
  node: WorkflowNode;
  workflow: Workflow;
}) => {
  const listRef = useRef<HTMLDivElement | null>(null);
  const [mode, setMode] = useState<string>("utf-8");
  const { msgdata, clearStep } = useMsgStore();

  const { items, sendStepIdSet, nodeNameById } = useMemo(() => {
    const sendStepIds = getRelatedSendStepIds(workflow, node.id);
    const sessionStepIdSet = new Set([node.id, ...sendStepIds]);
    const sendStepIdSet = new Set(sendStepIds);
    const nodeNameById = new Map(
      workflow.nodes.map((workflowNode) => [
        workflowNode.id,
        workflowNode.data.name || workflowNode.id,
      ]),
    );
    const items = msgdata
      .filter(
        (item) =>
          item.taskId === workflow.id && sessionStepIdSet.has(item.stepId),
      )
      .sort((a, b) => a.time - b.time);

    return { items, sendStepIdSet, nodeNameById };
  }, [msgdata, node.id, workflow]);

  const receiveCount = useMsgStore(
    (state) => state.msgcount[workflow.id]?.[node.id] ?? 0,
  );

  useEffect(() => {
    requestAnimationFrame(() => {
      const list = listRef.current;
      if (list) {
        list.scrollTop = list.scrollHeight;
      }
    });
  }, [items.length]);

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
        borderRadius: 8,
        boxShadow: "0 1px 4px rgba(15, 23, 42, 0.04)",
        minHeight: 0,
      }}
    >
      <Flex justify="space-between" align="center" gap={8}>
        <Typography.Text
          ellipsis
          style={{ color: "#1677ff", fontSize: 13, maxWidth: 180 }}
        >
          {node.data.name}
        </Typography.Text>
        <Space>
          <Segmented
            value={mode}
            options={["utf-8", "hex"]}
            onChange={(next) => setMode(next)}
          />
          <Button
            icon={<ClearOutlined />}
            disabled={receiveCount === 0}
            onClick={async () => {
              await clearStep(workflow.id, node.id);
            }}
          >
            <FormattedMessage
              id="work.output.clearWithCount"
              defaultMessage="清空 ({count})"
              values={{ count: receiveCount }}
            />
          </Button>
        </Space>
      </Flex>

      <div
        ref={listRef} //为了调整滚动条到最下面
        style={{
          flex: 1,
          minHeight: 0,
          overflow: "auto",
          marginTop: 10,
          paddingRight: 4,
          fontFamily:
            "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
          fontSize: 12,
        }}
      >
        {items.length === 0 ? (
          <Empty />
        ) : (
          items.map((item, index) => {
            const isSend = sendStepIdSet.has(item.stepBy);
            return (
              <div
                key={index}
                style={{
                  borderBottom: "1px solid #f0f0f0",
                  padding: "7px 0",
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                  display: "flex",
                  justifyContent: isSend ? "flex-end" : "flex-start",
                }}
              >
                <div
                  style={{
                    maxWidth: "78%",
                    padding: "6px 8px",
                    border: `1px solid ${isSend ? "#ffd8bf" : "#d6e4ff"}`,
                    borderRadius: 6,
                    background: isSend ? "#fff7e6" : "#f0f5ff",
                  }}
                >
                  <Flex justify="space-between" gap={8} wrap="wrap">
                    <Typography.Text type="secondary" style={{ fontSize: 11 }}>
                      {dayjs(item.time).format("YYYY-MM-DD HH:mm:ss.SSS")}
                    </Typography.Text>
                    <Typography.Text type="secondary" style={{ fontSize: 11 }}>
                      {nodeNameById.get(item.stepBy)} · {item.msg.length} B
                    </Typography.Text>
                  </Flex>
                  <div style={{ marginTop: 4 }}>
                    {mode === "hex"
                      ? bytesToHex(item.msg)
                      : textDecoder.decode(item.msg)}
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};

export default () => {
  const select = useWorkflowStore((state) => state.select);
  const nodes =
    select?.nodes.filter((node) => node.type === "DisOutputStep") ?? [];

  return nodes.length === 0 ? (
    <Empty />
  ) : (
    <Splitter style={{ height: "100%" }}>
      {nodes.map((node) => (
        <Splitter.Panel key={node.id}>
          <OutputPanel node={node} workflow={select} />
        </Splitter.Panel>
      ))}
    </Splitter>
  );
};
