import { create } from "zustand";
import { getStoreValue, setStoreValue } from "../appStore";

const WORKFLOWS_STORAGE_KEY = "workflows";
const SELECT_WORKFLOW_STORAGE_KEY = "workflow";

const createBlankWorkflow = (): Workflow => ({
  id: String(Date.now()),
  name: "New Blank",
  nodes: [],
  edges: [],
});

// 工作流数据只保存普通 JSON 字段，深拷贝后可避免编辑态和已保存模板共用同一份引用。
const cloneWorkflow = (workflow: Workflow): Workflow =>
  JSON.parse(JSON.stringify(workflow)) as Workflow;

/**修改变更状态 */
export const useWorkflowIsChange = () =>
  useWorkflowStore((state) => {
    const saved = state.workflows.find(
      (workflow) => workflow.id === state.select.id,
    );
    return !saved || JSON.stringify(saved) !== JSON.stringify(state.select);
  });

interface WorkflowState {
  select: Workflow;
  workflows: Workflow[];

  setSelect: (select: Workflow) => void;
  createWorkflow: (templateId?: string | null) => void;
  importWorkflow: (workflow: Workflow) => Workflow;
  save: () => void;
  remove: (id: string) => void;
  hydrate: () => Promise<void>;
}

export const useWorkflowStore = create<WorkflowState>((set, get) => ({
  select: createBlankWorkflow(),
  setSelect: (select) => {
    const selectedId = get().select.id;
    set({ select: cloneWorkflow(select) });
    if (select.id !== selectedId) {
      void setStoreValue(SELECT_WORKFLOW_STORAGE_KEY, select.id);
    }
  },

  workflows: [],

  createWorkflow: (templateId) => {
    let workflow = createBlankWorkflow();
    const { workflows } = get();
    if (templateId) {
      const formwf = workflows.find((w) => w.id === templateId)!;
      workflow = {
        ...formwf,
        id: String(Date.now()),
        name: `${formwf.name} (Copy)`,
      };
    }
    const next = [...workflows, workflow];
    set({
      select: workflow,
      workflows: next,
    });
    void setStoreValue(WORKFLOWS_STORAGE_KEY, next);
    void setStoreValue(SELECT_WORKFLOW_STORAGE_KEY, workflow.id);
  },

  importWorkflow: (workflow) => {
    const imported = cloneWorkflow({
      ...workflow,
      id: String(Date.now()),
    });
    const { workflows } = get();
    const next = [...workflows, imported];

    set({
      select: imported,
      workflows: next,
    });
    void setStoreValue(WORKFLOWS_STORAGE_KEY, next);
    void setStoreValue(SELECT_WORKFLOW_STORAGE_KEY, imported.id);

    return imported;
  },

  save: () => {
    const { select, workflows } = get();
    const saved = cloneWorkflow(select);
    const idx = workflows.findIndex((w) => w.id === select.id);
    const next =
      idx >= 0
        ? workflows.map((w) => (w.id === select.id ? saved : w))
        : [...workflows, saved];
    set({ workflows: next });
    void setStoreValue(WORKFLOWS_STORAGE_KEY, next);
  },

  remove: (id) => {
    const { workflows } = get();
    const kept = workflows.filter((w) => w.id !== id);
    const next = kept.length > 0 ? kept : [createBlankWorkflow()];
    const select = next[0];
    set({
      select: cloneWorkflow(select),
      workflows: next,
    });
    void setStoreValue(WORKFLOWS_STORAGE_KEY, next);
    void setStoreValue(SELECT_WORKFLOW_STORAGE_KEY, select.id);
  },

  hydrate: async () => {
    const stored = await getStoreValue<Workflow[]>(WORKFLOWS_STORAGE_KEY, []);
    const workflows =
      Array.isArray(stored) && stored.length > 0
        ? stored
        : [createBlankWorkflow()];
    const selectedId = await getStoreValue<string | undefined>(
      SELECT_WORKFLOW_STORAGE_KEY,
      undefined,
    );
    const select = workflows.find((w) => w.id === selectedId) ?? workflows[0];

    set({
      select: cloneWorkflow(select),
      workflows,
    });

    if (!Array.isArray(stored) || stored.length === 0) {
      void setStoreValue(WORKFLOWS_STORAGE_KEY, workflows);
    }
    if (select?.id) {
      void setStoreValue(SELECT_WORKFLOW_STORAGE_KEY, select.id);
    }
  },
}));

void useWorkflowStore.getState().hydrate();
