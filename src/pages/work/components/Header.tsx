import { EditOutlined, PauseCircleOutlined, PlayCircleOutlined } from "@ant-design/icons";
import { invoke } from "@tauri-apps/api/core";
import { App, Button, Flex, Space } from "antd";
import { FormattedMessage } from "react-intl";
import SelectWork from "../../SelectWork";
import {
  useWorkflowIsChange,
  useWorkflowStore,
} from "../../../models/workflow";
import { useActiveTabStore } from "../../../models/activeTab";
import { useWorkrunStore } from "../../../models/workrun";

export default () => {
  const { message, modal } = App.useApp();
  const select = useWorkflowStore((state) => state.select);
  const setActiveTab = useActiveTabStore((state) => state.setActiveTab);
  const { runningIds, addRunning, removeRunning } = useWorkrunStore();
  const isChange = useWorkflowIsChange();
  const isRunning = select ? runningIds.includes(select.id) : false;

  return (
    <Flex justify="space-between" style={{ margin: 10 }}>
      <SelectWork />
      <Space>
        {isRunning ? (
          <Button
            icon={<PauseCircleOutlined />}
            type="primary"
            danger
            disabled={!select}
            onClick={async () => {
              if (!select) return;
              try {
                await invoke("stop_workflow", { id: select.id });
                removeRunning(select.id);
                message.success(
                  <FormattedMessage
                    id="work.message.stopped"
                    defaultMessage="任务已停止：{workflowId}"
                    values={{ workflowId: select.id }}
                  />,
                );
              } catch (error) {
                message.error(String(error));
              }
            }}
          >
            <FormattedMessage id="work.action.stop" defaultMessage="停止" />
          </Button>
        ) : (
          <Button
            icon={<PlayCircleOutlined />}
            type="primary"
            disabled={!select}
            onClick={async () => {
              if (!select) return;
              try {
                const workflowId = await invoke<string>("start_workflow", {
                  json: JSON.stringify(select),
                });
                addRunning(workflowId);
                message.success(
                  <FormattedMessage
                    id="work.message.started"
                    defaultMessage="任务已启动：{workflowId}"
                    values={{ workflowId }}
                  />,
                );
              } catch (error) {
                message.error(String(error));
              }
            }}
          >
            <FormattedMessage id="work.action.start" defaultMessage="启动" />
          </Button>
        )}
        <Button
          icon={<EditOutlined />}
          onClick={() => {
            if (isChange) {
              modal.warning({
                content: (
                  <FormattedMessage
                    id="save.warning"
                    defaultMessage="请先保存，或者放弃重置"
                  />
                ),
              });
              return;
            }
            setActiveTab("designer");
          }}
        >
          <FormattedMessage id="work.action.settings" defaultMessage="设置" />
        </Button>
      </Space>
    </Flex>
  );
};
