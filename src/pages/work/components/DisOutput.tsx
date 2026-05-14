import { ClearOutlined } from "@ant-design/icons";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  App,
  Button,
  Empty,
  Flex,
  Segmented,
  Space,
  Spin,
  Splitter,
  Typography,
} from "antd";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useIntl } from "react-intl";
import { useWorkflowStore } from "../../../models/workflow";

const PAGE_SIZE = 100;

type OutputMode = "utf-8" | "hex";

type ReceiveLogRecord = {
  id: string;
  received_at: number;
  workflow_id: string;
  step_id: string;
  source_step_id: string;
  byte_len: number;
  msg: unknown;
};

const bytesToHex = (bytes: number[]) =>
  bytes
    .map((byte) => byte.toString(16).padStart(2, "0").toUpperCase())
    .join(" ");

const formatPayload = (msg: unknown, mode: OutputMode) => {
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

const mergeLogs = (
  current: ReceiveLogRecord[],
  incoming: ReceiveLogRecord[],
) => {
  const records = new Map<string, ReceiveLogRecord>();
  for (const item of current) {
    records.set(item.id, item);
  }
  for (const item of incoming) {
    records.set(item.id, item);
  }
  return [...records.values()].sort((a, b) => a.id.localeCompare(b.id));
};

function OutputPanel({ node, workflowId }: { node: WorkflowNode; workflowId: string }) {
  const { message } = App.useApp();
  const intl = useIntl();
  const listRef = useRef<HTMLDivElement | null>(null);
  const [mode, setMode] = useState<OutputMode>("utf-8");
  const [items, setItems] = useState<ReceiveLogRecord[]>([]);
  const [loadingInitial, setLoadingInitial] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [hasMore, setHasMore] = useState(false);

  const oldestCursor = items[0]?.id;

  const scrollToBottom = useCallback(() => {
    requestAnimationFrame(() => {
      const list = listRef.current;
      if (list) {
        list.scrollTop = list.scrollHeight;
      }
    });
  }, []);

  const loadInitial = useCallback(async () => {
    setLoadingInitial(true);
    try {
      const logs = await invoke<ReceiveLogRecord[]>("get_receive_logs", {
        workflowId,
        stepId: node.id,
        limit: PAGE_SIZE,
      });
      setItems(logs);
      setHasMore(logs.length === PAGE_SIZE);
      scrollToBottom();
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoadingInitial(false);
    }
  }, [message, node.id, scrollToBottom, workflowId]);

  const loadOlder = useCallback(async () => {
    if (!oldestCursor || loadingOlder || !hasMore) {
      return;
    }

    const list = listRef.current;
    const previousHeight = list?.scrollHeight ?? 0;
    setLoadingOlder(true);
    try {
      const logs = await invoke<ReceiveLogRecord[]>("get_receive_logs", {
        workflowId,
        stepId: node.id,
        before: oldestCursor,
        limit: PAGE_SIZE,
      });
      setItems((current) => mergeLogs(current, logs));
      setHasMore(logs.length === PAGE_SIZE);
      requestAnimationFrame(() => {
        const currentList = listRef.current;
        if (currentList) {
          currentList.scrollTop += currentList.scrollHeight - previousHeight;
        }
      });
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoadingOlder(false);
    }
  }, [hasMore, loadingOlder, message, node.id, oldestCursor, workflowId]);

  useEffect(() => {
    setItems([]);
    setHasMore(false);
    void loadInitial();
  }, [loadInitial]);

  useEffect(() => {
    const unlisten = listen<ReceiveLogRecord>(
      "workflow-step-message",
      ({ payload }) => {
        if (payload.workflow_id !== workflowId || payload.step_id !== node.id) {
          return;
        }
        setItems((current) => mergeLogs(current, [payload]));
        scrollToBottom();
      },
    );

    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, [node.id, scrollToBottom, workflowId]);

  const emptyText = useMemo(
    () =>
      intl.formatMessage({
        id: "work.output.emptyLogs",
        defaultMessage: "暂无接收数据",
      }),
    [intl],
  );

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
            onChange={(next) => setMode(next as OutputMode)}
          />
          <Button
            icon={<ClearOutlined />}
            disabled={items.length === 0}
            onClick={async () => {
              try {
                await invoke("clear_receive_logs", {
                  workflowId,
                  stepId: node.id,
                });
                setItems([]);
                setHasMore(false);
              } catch (error) {
                message.error(String(error));
              }
            }}
          >
            {intl.formatMessage({
              id: "work.output.clear",
              defaultMessage: "清空",
            })}
          </Button>
        </Space>
      </Flex>

      <div
        ref={listRef}
        onScroll={(event) => {
          if (event.currentTarget.scrollTop <= 8) {
            void loadOlder();
          }
        }}
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
        {loadingOlder && (
          <Flex justify="center" style={{ padding: "8px 0" }}>
            <Spin size="small" />
          </Flex>
        )}
        {loadingInitial ? (
          <Flex justify="center" align="center" style={{ height: "100%" }}>
            <Spin />
          </Flex>
        ) : items.length === 0 ? (
          <Flex justify="center" align="center" style={{ height: "100%" }}>
            <Typography.Text type="secondary">{emptyText}</Typography.Text>
          </Flex>
        ) : (
          items.map((item) => (
            <div
              key={item.id}
              style={{
                borderBottom: "1px solid #f0f0f0",
                padding: "7px 0",
                whiteSpace: "pre-wrap",
                wordBreak: "break-word",
              }}
            >
              <Flex justify="space-between" gap={8} wrap="wrap">
                <Typography.Text type="secondary" style={{ fontSize: 11 }}>
                  {new Date(item.received_at).toLocaleString()}
                </Typography.Text>
                <Typography.Text type="secondary" style={{ fontSize: 11 }}>
                  {item.source_step_id} · {item.byte_len} B
                </Typography.Text>
              </Flex>
              <div style={{ marginTop: 4 }}>{formatPayload(item.msg, mode)}</div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export default () => {
  const select = useWorkflowStore((state) => state.select);
  const intl = useIntl();
  const workflowId = select?.id;
  const nodes = select?.nodes.filter((node) => node.type === "DisOutputStep") ?? [];

  if (nodes.length === 0 || !workflowId) {
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
          <OutputPanel node={node} workflowId={workflowId} />
        </Splitter.Panel>
      ))}
    </Splitter>
  );
};
