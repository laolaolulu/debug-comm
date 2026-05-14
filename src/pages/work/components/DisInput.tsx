import { SendOutlined } from "@ant-design/icons";
import { invoke } from "@tauri-apps/api/core";
import {
  App,
  Button,
  Empty,
  Flex,
  Input,
  Segmented,
  Space,
  Splitter,
  Typography,
} from "antd";
import { useState } from "react";
import { FormattedMessage, useIntl } from "react-intl";
import { useWorkflowStore } from "../../../models/workflow";

const parseHex = (
  value: string,
  messages: { oddLength: string; invalidByte: (part: string) => string },
) => {
  const normalized = value.replace(/[\s,-]/g, "");
  if (!normalized) {
    return [];
  }
  if (normalized.length % 2 !== 0) {
    throw new Error(messages.oddLength);
  }
  return (
    normalized.match(/.{2}/g)?.map((part) => {
      const byte = Number.parseInt(part, 16);
      if (Number.isNaN(byte)) {
        throw new Error(messages.invalidByte(part));
      }
      return byte;
    }) ?? []
  );
};

function InputPanel({ node }: { node: WorkflowNode }) {
  const { message } = App.useApp();
  const select = useWorkflowStore((state) => state.select);
  const intl = useIntl();
  const [mode, setMode] = useState<"utf-8" | "hex">("utf-8");
  const [value, setValue] = useState("");

  return (
    <div
      style={{
        height: "calc(100% - 20px)",
        display: "flex",
        flexDirection: "column",
        margin: 10,
        padding: "10px 12px",
        background: "#fff",
        border: "1px solid #d9d9d9",
        borderRadius: 15,
        boxShadow: "0 1px 4px rgba(15, 23, 42, 0.04)",
      }}
    >
      <Input.TextArea
        variant="borderless"
        value={value}
        placeholder={
          mode === "hex"
            ? "AA 01 FF"
            : intl.formatMessage({
                id: "work.input.placeholder.text",
                defaultMessage: "请输入发送内容",
              })
        }
        onChange={(event) => setValue(event.target.value)}
        style={{ flex: 1, padding: 0, resize: "none" }}
      />

      <Flex justify="space-between" align="flex-end">
        <Typography.Text
          ellipsis
          style={{ color: "#fa541c", fontSize: 13, maxWidth: 160 }}
        >
          {node.data.name}
        </Typography.Text>
        <Space>
          <Segmented
            value={mode}
            options={["utf-8", "hex"]}
            onChange={(next) => setMode(next as "utf-8" | "hex")}
          />
          <Button
            type="primary"
            icon={<SendOutlined />}
            disabled={!value.trim() || !select}
            onClick={async () => {
              if (!select) {
                return;
              }
              try {
                await invoke("publish_step_message", {
                  workflowId: select.id,
                  stepId: node.id,
                  msg:
                    mode === "hex"
                      ? parseHex(value, {
                          oddLength: intl.formatMessage({
                            id: "work.input.error.hexOddLength",
                            defaultMessage: "HEX 长度必须是偶数",
                          }),
                          invalidByte: (part) =>
                            intl.formatMessage(
                              {
                                id: "work.input.error.invalidHexByte",
                                defaultMessage: "非法 HEX 字节：{part}",
                              },
                              { part },
                            ),
                        })
                      : value,
                });
                message.success(
                  intl.formatMessage({
                    id: "work.input.sent",
                    defaultMessage: "已发送",
                  }),
                );
              } catch (error) {
                message.error(String(error));
              }
            }}
          >
            <FormattedMessage id="work.input.send" defaultMessage="发送" />
          </Button>
        </Space>
      </Flex>
    </div>
  );
}

export default () => {
  const select = useWorkflowStore((state) => state.select);
  const intl = useIntl();
  const nodes = select?.nodes.filter((f) => f.type === "DisInputStep") ?? [];

  if (nodes.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={intl.formatMessage({
          id: "work.input.noNodes",
          defaultMessage: "暂无输入节点",
        })}
      />
    );
  }

  return (
    <Splitter style={{ height: "100%" }}>
      {nodes.map((node) => (
        <Splitter.Panel key={node.id}>
          <InputPanel node={node} />
        </Splitter.Panel>
      ))}
    </Splitter>
  );
};
