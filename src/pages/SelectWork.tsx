import { App, Badge, Select } from "antd";
import { useWorkflowIsChange, useWorkflowStore } from "../models/workflow";
import { useWorkrunStore } from "../models/workrun";
import { FormattedMessage } from "react-intl";

const renderWorkflowLabel = (name: string, isRunning: boolean) => (
  <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
    <Badge status={isRunning ? "processing" : "default"} />
    <span>{name}</span>
  </span>
);

export default () => {
  const { select, workflows, setSelect } = useWorkflowStore();
  const { runningIds } = useWorkrunStore();
  const { modal } = App.useApp();
  const isChange = useWorkflowIsChange();

  return (
    <Select
      value={select?.id}
      style={{ width: 200 }}
      options={workflows.map((m) => ({
        value: m.id,
        label: renderWorkflowLabel(m.name, runningIds.includes(m.id)),
      }))}
      onChange={(id) => {
        if (isChange) {
          modal.warning({
            content: (
              <FormattedMessage
                id="save.warning"
                defaultMessage="请先保存，或者放弃重置"
              />
            ),
          });
        } else {
          setSelect(workflows.find((f) => f.id === id)!);
        }
      }}
    />
  );
};
