import {
  BetaSchemaForm,
  ProFormColumnsType,
  ProFormInstance,
} from "@ant-design/pro-components";
import { DeleteOutlined } from "@ant-design/icons";
import { useReactFlow } from "@xyflow/react";
import { Button, Flex, Typography } from "antd";
import { useCallback, useEffect, useMemo, useRef } from "react";
import { FormattedMessage, useIntl } from "react-intl";
import { nodeType } from "..";
import { useWorkflowStore } from "../../../models/workflow";
type FieldPath = (string | number)[];
type FormValues = {
  id?: string;
  name?: string;
  description?: string;
  nodes?: Record<string, Record<string, unknown>>;
};

const getFieldPath = (dataIndex: unknown): FieldPath | undefined => {
  if (Array.isArray(dataIndex)) {
    return dataIndex.filter(
      (item): item is string | number =>
        typeof item === "string" || typeof item === "number",
    );
  }
  if (typeof dataIndex === "string" || typeof dataIndex === "number") {
    return [dataIndex];
  }
  return undefined;
};

const getValueByPath = (source: Record<string, unknown>, path: FieldPath) =>
  path.reduce<unknown>((value, key) => {
    if (value && typeof value === "object") {
      return (value as Record<string | number, unknown>)[key];
    }
    return undefined;
  }, source);

const withNodeDataIndex = (
  columns: ProFormColumnsType[],
  node: WorkflowNode,
): ProFormColumnsType[] =>
  columns.map((column) => {
    const fieldPath = getFieldPath(column.dataIndex);
    const nextColumn: ProFormColumnsType = { ...column };

    if (fieldPath) {
      nextColumn.dataIndex = ["nodes", node.id, ...fieldPath];
      nextColumn.initialValue =
        getValueByPath(node.data, fieldPath) ?? column.initialValue;
    }

    if (Array.isArray(column.columns)) {
      nextColumn.columns = withNodeDataIndex(
        column.columns as ProFormColumnsType[],
        node,
      );
    }

    return nextColumn;
  });

const getFormValues = (workflow: Workflow): FormValues => ({
  id: workflow.id,
  name: workflow.name,
  description: workflow.description,
  nodes: Object.fromEntries(workflow.nodes.map((node) => [node.id, node.data])),
});

export default () => {
  const { select, setSelect } = useWorkflowStore();
  const { deleteElements } = useReactFlow();
  const formRef = useRef<ProFormInstance<FormValues>>(null);
  const intl = useIntl();

  useEffect(() => {
    formRef.current?.setFieldsValue(getFormValues(select));
  }, [select]);

  const handleDeleteNode = useCallback(
    (nodeId: string) => {
      deleteElements({ nodes: [{ id: nodeId }] });
    },
    [deleteElements],
  );

  const handleValuesChange = useCallback(
    (_: unknown, values: FormValues) => {
      const nodeValues = values.nodes ?? {};
      const nextNodes = select.nodes.map((node) => {
        const patch = nodeValues[node.id];
        if (!patch) {
          return node;
        }

        return {
          ...node,
          data: {
            ...node.data,
            ...patch,
          },
        };
      });

      setSelect({
        ...select,
        name: typeof values.name === "string" ? values.name : select.name,
        description:
          typeof values.description === "string"
            ? values.description
            : select.description,
        nodes: nextNodes,
      });
    },
    [select, setSelect],
  );

  const columns: ProFormColumnsType[] = useMemo(() => {
    const from: ProFormColumnsType[] = [
      {
        title: (
          <Flex
            justify="space-between"
            style={{ margin: "15px 15px 10px 15px" }}
          >
            <Typography.Text strong>
              <FormattedMessage
                id="flow.stepPar.taskInfo"
                defaultMessage="任务信息"
              />
            </Typography.Text>
            <Typography.Text type="secondary">
              <FormattedMessage
                id="flow.stepPar.selectedCount"
                defaultMessage="选中 {count} 个"
                values={{
                  count: select.nodes.filter((f) => f.selected).length,
                }}
              />
            </Typography.Text>
          </Flex>
        ),
        valueType: "group",
        columns: [
          {
            title: intl.formatMessage({
              id: "flow.stepPar.taskId",
              defaultMessage: "任务编号",
            }),
            dataIndex: "id",
            hideInForm: true,
            initialValue: select.id,
          },
          {
            title: intl.formatMessage({
              id: "flow.stepPar.taskName",
              defaultMessage: "任务名称",
            }),
            dataIndex: "name",
            initialValue: select.name,
          },
          {
            title: intl.formatMessage({
              id: "flow.stepPar.taskDescription",
              defaultMessage: "任务描述",
            }),
            dataIndex: "description",
            valueType: "textarea",
            initialValue: select.description,
          },
        ],
      },
    ];
    select.nodes
      .filter((f) => f.selected)
      .forEach((m) => {
        const addcolumns: ProFormColumnsType[] = [
          {
            title: intl.formatMessage({
              id: "flow.stepPar.nodeName",
              defaultMessage: "节点名称",
            }),
            dataIndex: ["nodes", m.id, "name"],
            initialValue: m.data.name,
          },
          {
            title: intl.formatMessage({
              id: "flow.stepPar.nodeDescription",
              defaultMessage: "节点描述",
            }),
            valueType: "textarea",
            dataIndex: ["nodes", m.id, "description"],
            initialValue: m.data.description,
          },
        ];
        from.push({
          title: (
            <Flex
              justify="space-between"
              style={{ margin: "15px 15px 10px 15px" }}
            >
              <Typography.Text strong>
                {intl.formatMessage(nodeType[m.type as keyof typeof nodeType])}
              </Typography.Text>
              <Button
                danger
                type="text"
                size="small"
                icon={<DeleteOutlined />}
                onClick={() => handleDeleteNode(m.id)}
              >
                <FormattedMessage
                  id="flow.stepPar.delete"
                  defaultMessage="删除"
                />
              </Button>
            </Flex>
          ),
          valueType: "group",
          columns: [
            ...addcolumns,
            ...withNodeDataIndex(m.data.columns ?? [], m),
          ],
        });
      });
    return from;
  }, [handleDeleteNode, intl, select]);

  return (
    <BetaSchemaForm
      formRef={formRef}
      key={select.id}
      grid
      style={{
        overflowX: "hidden",
        height: "calc(100vh - 100px)",
        scrollbarWidth: "thin",
      }}
      submitter={false}
      columns={columns}
      onValuesChange={handleValuesChange}
    />
  );
};
