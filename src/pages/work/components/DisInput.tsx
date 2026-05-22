import { ClearOutlined, SendOutlined } from '@ant-design/icons';
import { invoke } from '@tauri-apps/api/core';
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
} from 'antd';
import { useState } from 'react';
import { FormattedMessage, useIntl } from 'react-intl';
import { useMsgStore } from '../../../models/msgstore';
import { useWorkflowStore } from '../../../models/workflow';

export const payloadToBytes = (value: unknown): Uint8Array => {
  if (Array.isArray(value)) {
    return new Uint8Array(
      value.filter((byte): byte is number => typeof byte === 'number'),
    );
  }

  if (typeof value === 'string') {
    return new TextEncoder().encode(value);
  }

  // IndexedDB 只保存 byte[]，这样 UTF-8/HEX 两种显示模式能复用同一份数据。
  // 对象类消息先序列化为 JSON 文本，再按 UTF-8 字节保存。
  return new TextEncoder().encode(JSON.stringify(value));
};

type InputMode = 'utf-8' | 'hex';

const HEX_SEPARATOR_PATTERN = /[\s,;:\-_]/g;
const HEX_ALLOWED_PATTERN = /[^0-9a-fA-F\s,;:\-_]/g;

const bytesToHexInput = (bytes: Uint8Array) =>
  [...bytes]
    .map((byte) => byte.toString(16).padStart(2, '0').toUpperCase())
    .join(' ');

const sanitizeHexInput = (value: string) =>
  value.replace(HEX_ALLOWED_PATTERN, '');

const parseHex = (
  value: string,
  messages: { oddLength: string; invalidByte: (part: string) => string },
) => {
  const normalized = value.replace(HEX_SEPARATOR_PATTERN, '');
  if (!normalized) {
    return [];
  }
  if (normalized.length % 2 !== 0) {
    throw new Error(messages.oddLength);
  }
  if (!/^[0-9a-fA-F]+$/.test(normalized)) {
    throw new Error(messages.invalidByte(normalized));
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

const decodeHex = (
  value: string,
  messages: { oddLength: string; invalidByte: (part: string) => string },
) => new TextDecoder().decode(new Uint8Array(parseHex(value, messages)));

const InputPanel = ({ node }: { node: WorkflowNode }) => {
  const { message } = App.useApp();
  const select = useWorkflowStore((state) => state.select);
  const intl = useIntl();
  const [mode, setMode] = useState<InputMode>('utf-8');
  const [value, setValue] = useState('');
  const appendMessage = useMsgStore((state) => state.appendMessage);
  const clearStep = useMsgStore((state) => state.clearStep);
  const messageCount = useMsgStore((state) =>
    select ? (state.msgcount[select.id]?.[node.id] ?? 0) : 0,
  );
  const hexMessages = {
    oddLength: intl.formatMessage({
      id: 'work.input.error.hexOddLength',
      defaultMessage: 'HEX 长度必须是偶数',
    }),
    invalidByte: (part: string) =>
      intl.formatMessage(
        {
          id: 'work.input.error.invalidHexByte',
          defaultMessage: '非法 HEX 字节：{part}',
        },
        { part },
      ),
  };

  const switchMode = (next: InputMode) => {
    if (next === mode) {
      return;
    }

    try {
      if (next === 'hex') {
        setValue(bytesToHexInput(new TextEncoder().encode(value)));
      } else {
        setValue(decodeHex(value, hexMessages));
      }
      setMode(next);
    } catch (error) {
      message.error(String(error));
    }
  };

  return (
    <div
      style={{
        height: 'calc(100% - 20px)',
        display: 'flex',
        flexDirection: 'column',
        margin: 10,
        padding: '10px 12px',
        background: '#fff',
        border: '1px solid #d9d9d9',
        borderRadius: 8,
        boxShadow: '0 1px 4px rgba(15, 23, 42, 0.04)',
      }}
    >
      <Input.TextArea
        variant='borderless'
        value={value}
        placeholder={
          mode === 'hex'
            ? 'A1 B2 C3 或 A1-B2-C3'
            : intl.formatMessage({
                id: 'work.input.placeholder.text',
                defaultMessage: '请输入发送内容',
              })
        }
        onChange={(event) =>
          setValue(
            mode === 'hex'
              ? sanitizeHexInput(event.target.value)
              : event.target.value,
          )
        }
        style={{ flex: 1, padding: 0, resize: 'none' }}
      />

      <Flex justify='space-between' align='flex-end'>
        <Typography.Text
          ellipsis
          style={{ color: '#fa541c', fontSize: 13, maxWidth: 160 }}
        >
          {node.data.name}
        </Typography.Text>
        <Space>
          <Segmented
            value={mode}
            options={['utf-8', 'hex']}
            onChange={(next) => switchMode(next as InputMode)}
          />
          <Button
            icon={<ClearOutlined />}
            disabled={!select || messageCount === 0}
            onClick={async () => {
              if (!select) {
                return;
              }
              try {
                await clearStep(select.id, node.id);
              } catch (error) {
                message.error(String(error));
              }
            }}
          >
            {intl.formatMessage(
              {
                id: 'work.input.clearWithCount',
                defaultMessage: '清空 ({count})',
              },
              { count: messageCount },
            )}
          </Button>
          <Button
            type='primary'
            icon={<SendOutlined />}
            disabled={!value.trim() || !select}
            onClick={async () => {
              if (!select) {
                return;
              }
              try {
                const payload =
                  mode === 'hex' ? parseHex(value, hexMessages) : value;
                await invoke('publish_step_message', {
                  workflowId: select.id,
                  stepId: node.id,
                  msg: payload,
                });
                await appendMessage({
                  taskId: select.id,
                  stepId: node.id,
                  stepBy: node.id,
                  msg: payloadToBytes(payload),
                  time: Date.now(),
                });
                message.success(
                  intl.formatMessage({
                    id: 'work.input.sent',
                    defaultMessage: '已发送',
                  }),
                );
              } catch (error) {
                message.error(String(error));
              }
            }}
          >
            <FormattedMessage id='work.input.send' defaultMessage='发送' />
          </Button>
        </Space>
      </Flex>
    </div>
  );
};

export default () => {
  const select = useWorkflowStore((state) => state.select);
  const intl = useIntl();
  const nodes =
    select?.nodes.filter((node) => node.type === 'DisInputStep') ?? [];

  if (nodes.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={intl.formatMessage({
          id: 'work.input.noNodes',
          defaultMessage: '暂无输入节点',
        })}
      />
    );
  }

  return (
    <Splitter style={{ height: '100%' }}>
      {nodes.map((node) => (
        <Splitter.Panel key={node.id}>
          <InputPanel node={node} />
        </Splitter.Panel>
      ))}
    </Splitter>
  );
};
