import {
  DeleteOutlined,
  DownloadOutlined,
  DownOutlined,
  FileOutlined,
  FileSyncOutlined,
  FileTextOutlined,
  FolderAddOutlined,
  SaveOutlined,
  UploadOutlined,
} from "@ant-design/icons";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import {
  App,
  Button,
  Dropdown,
  Flex,
  MenuProps,
  Space,
  Upload,
} from "antd";
import { useCallback, useMemo } from "react";
import { FormattedMessage } from "react-intl";
import SelectWork from "../../SelectWork";
import {
  useWorkflowIsChange,
  useWorkflowStore,
} from "../../../models/workflow";

export default () => {
  const { message, modal } = App.useApp();
  const {
    select,
    workflows,
    setSelect,
    createWorkflow,
    importWorkflow,
    save,
    remove,
  } = useWorkflowStore();
  const isChange = useWorkflowIsChange();

  const UploadFile = useCallback(
    async (file: File) => {
      if (!file.name.toLowerCase().endsWith(".json")) {
        message.error(
          <FormattedMessage
            id="flow.header.invalidJsonFile"
            defaultMessage="文件【{name}】非json格式,已忽略"
            values={{ name: file.name }}
          />,
        );
        return Upload.LIST_IGNORE;
      }

      try {
        const content = await file.text();
        const parsed = JSON.parse(content) as Workflow;
        const imported = importWorkflow(parsed);
        message.success(
          <FormattedMessage
            id="flow.header.importSuccess"
            defaultMessage="文件【{name}】导入成功"
            values={{ name: imported.name }}
          />,
        );
      } catch (error) {
        message.error(
          <FormattedMessage
            id="flow.header.parseError"
            defaultMessage="文件解析失败"
          />,
        );
        console.error(error);
      }

      return Upload.LIST_IGNORE;
    },
    [importWorkflow, message],
  );

  const saveitems: MenuProps["items"] = useMemo(
    () => [
      {
        key: "export",
        icon: <DownloadOutlined />,
        label: <FormattedMessage id="step.save.export" defaultMessage="导出" />,
        onClick: async () => {
          const jsonStr = JSON.stringify(select, null, "\t");
          const filePath = await saveDialog({
            defaultPath: `${select.name}.json`,
            filters: [
              {
                name: "JSON",
                extensions: ["json"],
              },
            ],
          });

          if (!filePath) return;

          try {
            await writeTextFile(filePath, jsonStr);
            message.success(
              <FormattedMessage
                id="step.save.exportSuccess"
                defaultMessage="已导出：{name}"
                values={{ name: select.name }}
              />,
            );
          } catch (error) {
            message.error(
              <FormattedMessage
                id="step.save.exportError"
                defaultMessage="导出失败"
              />,
            );
            console.error(error);
          }
        },
      },
      { type: "divider" },
      {
        key: "reset",
        icon: <FileSyncOutlined />,
        disabled: !isChange,
        label: <FormattedMessage id="step.save.reset" defaultMessage="重置" />,
        onClick: () => {
          const saved = workflows.find((w) => w.id === select.id);
          if (saved) {
            setSelect(saved);
          }
          message.success(
            <FormattedMessage
              id="step.save.resetSuccess"
              defaultMessage="已重置：{name}"
              values={{ name: select.name }}
            />,
          );
        },
      },
      { type: "divider" },
      {
        key: "delete",
        label: (
          <FormattedMessage
            id="step.save.deleteTemplate"
            defaultMessage="删除模板"
          />
        ),
        icon: <DeleteOutlined />,
        onClick: async () => {
          const confirmed = await modal.confirm({
            title: (
              <FormattedMessage
                id="step.save.deleteWarning"
                defaultMessage="删除警告"
              />
            ),
            content: (
              <FormattedMessage
                id="step.save.deleteConfirm"
                defaultMessage="是否删除标签模板【{name}】"
                values={{ name: select.name }}
              />
            ),
          });
          if (confirmed) {
            remove(select.id);
            message.success(
              <FormattedMessage
                id="step.save.deleteSuccess"
                defaultMessage="已删除：{name}"
                values={{ name: select.name }}
              />,
            );
          }
        },
      },
    ],
    [select, workflows, isChange, setSelect, remove, message, modal],
  );
  const createitems: MenuProps["items"] = useMemo(
    () => [
      {
        key: "empty",
        label: (
          <FormattedMessage id="step.create.empty" defaultMessage="从空白页" />
        ),
        icon: <FileOutlined />,
        onClick: () => createWorkflow(),
      },
      { type: "divider" },
      {
        key: "newtem",
        label: (
          <FormattedMessage
            id="step.create.currentTemplate"
            defaultMessage="从当前模板"
          />
        ),
        icon: <FileTextOutlined />,
        onClick: () => createWorkflow(select.id),
      },
      { type: "divider" },
      {
        key: "import",
        label: (
          <Upload
            multiple
            showUploadList={false}
            beforeUpload={UploadFile}
            accept=".json"
          >
            <FormattedMessage
              id="step.create.importFile"
              defaultMessage="从文件导入"
            />
          </Upload>
        ),
        icon: <UploadOutlined />,
      },
    ],
    [UploadFile, createWorkflow, select],
  );

  return (
    <Flex justify="space-between" style={{ margin: 10 }}>
      <Space>
        <SelectWork />
        <Dropdown
          menu={{
            items: createitems,
          }}
        >
          <Button
            icon={<DownOutlined />}
            onClick={() => undefined}
            iconPlacement="end"
          >
            <FormattedMessage id="step.action.new" defaultMessage="新建" />
            <FolderAddOutlined />
          </Button>
        </Dropdown>
      </Space>
      <Space.Compact>
        <Button
          key="save"
          icon={<SaveOutlined />}
          disabled={!isChange}
          onClick={() => save()}
        >
          <FormattedMessage id="step.action.save" defaultMessage="保存" />
        </Button>
        <Dropdown
          menu={{
            items: saveitems,
          }}
          placement="bottomRight"
        >
          <Button icon={<DownOutlined />} />
        </Dropdown>
      </Space.Compact>
    </Flex>
  );
};
