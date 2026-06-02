import {
  EditOutlined,
  PauseCircleOutlined,
  PlayCircleOutlined,
} from '@ant-design/icons';
import { invoke } from '@tauri-apps/api/core';
import { App, Button, Flex, Space } from 'antd';
import { FormattedMessage } from 'react-intl';
import SelectWork from '../../SelectWork';
import {
  useWorkflowIsChange,
  useWorkflowStore,
} from '../../../models/workflow';
import { useActiveTabStore } from '../../../models/activeTab';
import { useWorkrunStore } from '../../../models/workrun';

// 渲染工作台头部操作栏。
export default () => {
  const { message, modal } = App.useApp();
  const select = useWorkflowStore((state) => state.select);
  const setActiveTab = useActiveTabStore((state) => state.setActiveTab);
  const { runningIds, addRunning, removeRunning } = useWorkrunStore();
  const isChange = useWorkflowIsChange();
  const isRunning = runningIds.includes(select.id);

  return (
    <Flex justify='space-between' style={{ margin: 10 }}>
      <SelectWork />
      <Space>
        {isRunning ? (
          <Button
            icon={<PauseCircleOutlined />}
            type='primary'
            danger
            onClick={async () => {
              try {
                await invoke('stop_workflow', { id: select.id });
                removeRunning(select.id);
                message.success(
                  <FormattedMessage
                    id='work.message.stopped'
                    defaultMessage='任务已停止：{workflowId}'
                    values={{ workflowId: select.id }}
                  />,
                );
              } catch (error) {
                message.error(String(error));
              }
            }}
          >
            <FormattedMessage id='work.action.stop' defaultMessage='停止' />
          </Button>
        ) : (
          <Button
            icon={<PlayCircleOutlined />}
            type='primary'
            onClick={async () => {
              try {
                await invoke<void>('start_workflow', {
                  json: JSON.stringify(select),
                });
                addRunning(select.id);
                message.success(
                  <FormattedMessage
                    id='work.message.started'
                    defaultMessage='任务已启动：{workflowId}'
                    values={{ workflowId: select.id }}
                  />,
                );
              } catch (error) {
                message.error(String(error));
              }
            }}
          >
            <FormattedMessage id='work.action.start' defaultMessage='启动' />
          </Button>
        )}
        <Button
          icon={<EditOutlined />}
          onClick={() => {
            if (isChange) {
              modal.warning({
                content: (
                  <FormattedMessage
                    id='save.warning'
                    defaultMessage='请先保存，或者放弃重置'
                  />
                ),
              });
              return;
            }
            setActiveTab('designer');
          }}
        >
          <FormattedMessage id='work.action.settings' defaultMessage='设置' />
        </Button>
      </Space>
    </Flex>
  );
};
