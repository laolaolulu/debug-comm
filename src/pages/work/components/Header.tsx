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

const renderStartErrors = (errors: WorkflowStartError[]) => (
  <div style={{ maxHeight: 360, overflow: 'auto' }}>
    <FormattedMessage
      id='work.startErrors.description'
      defaultMessage='The following nodes failed to start:'
    />
    <ul style={{ margin: '8px 0 0', paddingLeft: 20 }}>
      {errors.map((error) => (
        <li key={error.stepId} style={{ marginBottom: 8 }}>
          <div>
            <strong>{error.stepName}</strong>
            <span>{` (${error.stepType}, ${error.stepId})`}</span>
          </div>
          <div style={{ color: '#cf1322', wordBreak: 'break-word' }}>
            {error.message}
          </div>
        </li>
      ))}
    </ul>
  </div>
);

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
                const result = await invoke<WorkflowStartResult>('start_workflow', {
                  json: JSON.stringify(select),
                });
                if (result.errors.length === 0) {
                  if (result.started) {
                    addRunning(select.id);
                    message.success(
                      <FormattedMessage
                        id='work.message.started'
                        defaultMessage='Task started: {workflowId}'
                        values={{ workflowId: select.id }}
                      />,
                    );
                  } else {
                    message.error(
                      <FormattedMessage
                        id='work.message.noStartedNode'
                        defaultMessage='No node was started'
                      />,
                    );
                  }
                  return;
                }

                if (!result.started) {
                  modal.error({
                    title: (
                      <FormattedMessage
                        id='work.startErrors.failedTitle'
                        defaultMessage='Task start failed'
                      />
                    ),
                    content: renderStartErrors(result.errors),
                  });
                  return;
                }

                const confirmed = await modal.confirm({
                  title: (
                    <FormattedMessage
                      id='work.startErrors.confirmTitle'
                      defaultMessage='Some nodes failed to start. Continue?'
                    />
                  ),
                  content: renderStartErrors(result.errors),
                  okText: (
                    <FormattedMessage
                      id='work.startErrors.continue'
                      defaultMessage='Continue'
                    />
                  ),
                  cancelText: (
                    <FormattedMessage
                      id='work.startErrors.stop'
                      defaultMessage='No'
                    />
                  ),
                });

                if (confirmed) {
                  addRunning(select.id);
                  message.success(
                    <FormattedMessage
                      id='work.message.startedWithErrors'
                      defaultMessage='Task started with some failed nodes: {workflowId}'
                      values={{ workflowId: select.id }}
                    />,
                  );
                } else {
                  await invoke('stop_workflow', { id: select.id });
                  removeRunning(select.id);
                  message.success(
                    <FormattedMessage
                      id='work.message.startCancelled'
                      defaultMessage='Task ended: {workflowId}'
                      values={{ workflowId: select.id }}
                    />,
                  );
                }
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
