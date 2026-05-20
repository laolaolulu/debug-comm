import { listen } from '@tauri-apps/api/event';
import Dexie, { type Table } from 'dexie';
import { create } from 'zustand';

const DB_NAME = 'debug-com-msg';
const OLDER_PAGE_SIZE = 100;

export type MsgData = {
  taskId: string;
  stepId: string;
  stepBy: string;
  msg: number[];
  time: number;
};

type StoredMsgData = MsgData & {
  id?: number;
};

class MsgDatabase extends Dexie {
  msgdata!: Table<StoredMsgData, number>;

  constructor() {
    super(DB_NAME);

    // `++` 使用 autoIncrement 主键，MsgData 不需要暴露业务 id。
    this.version(1).stores({
      msgdata: '++,by_task_step_time,[taskId+stepId+time]',
    });
  }
}

type MsgState = {
  /** 当前加载到前端内存里的消息列表，界面按 taskId/stepId 自己筛选。 */
  msgdata: MsgData[];
  /** IndexedDB 中每个 taskId + stepId 的消息总数，用于清空按钮计数。*/
  msgcount: Record<string, Record<string, number>>;
  /** 应用启动时调用一次：注册 Tauri 消息监听，并恢复每组最新消息。*/
  hydrate: () => Promise<void>;
  /** 统一写入入口：写 IndexedDB 后同步追加到内存 state。*/
  appendMessage: (record: MsgData) => Promise<void>;
  /** 清空单个 step 的消息。*/
  clearStep: (taskId: string, stepId: string) => Promise<void>;
};

const db = new MsgDatabase();

const readLatestMessagesGroupByStep = async (limit: number) => {
  const rows: Record<string, Record<string, MsgData[]>> = {};
  const counts: Record<string, Record<string, number>> = {};

  await db.msgdata
    .orderBy('[taskId+stepId+time]')
    .reverse()
    .each(({ taskId, stepId, stepBy, msg, time }) => {
      counts[taskId] = counts[taskId] ?? {};
      counts[taskId][stepId] = (counts[taskId][stepId] ?? 0) + 1;

      rows[taskId] = rows[taskId] ?? {};
      rows[taskId][stepId] = rows[taskId][stepId] ?? [];

      if (rows[taskId][stepId].length < limit) {
        rows[taskId][stepId].unshift({
          taskId,
          stepId,
          stepBy: stepBy ?? stepId,
          msg,
          time,
        });
      }
    });

  return {
    msgdata: Object.values(rows).flatMap((stepMap) =>
      Object.values(stepMap).flat(),
    ),
    counts,
  };
};

export const useMsgStore = create<MsgState>((set, get) => ({
  msgdata: [],
  msgcount: {},
  appendMessage: async (record) => {
    await db.msgdata.add(record);
    set((state) => {
      const current = state.msgdata.filter(
        (item) =>
          item.taskId === record.taskId && item.stepId === record.stepId,
      );
      return {
        msgdata: [...state.msgdata, record].sort((a, b) => a.time - b.time),
        msgcount: {
          ...state.msgcount,
          [record.taskId]: {
            ...(state.msgcount[record.taskId] ?? {}),
            [record.stepId]:
              (state.msgcount[record.taskId]?.[record.stepId] ??
                current.length) + 1,
          },
        },
      };
    });
  },

  clearStep: async (taskId, stepId) => {
    await db.msgdata
      .where('[taskId+stepId+time]')
      .between([taskId, stepId, Dexie.minKey], [taskId, stepId, Dexie.maxKey])
      .delete();
    set((state) => ({
      msgdata: state.msgdata.filter(
        (item) => item.taskId !== taskId || item.stepId !== stepId,
      ),
      msgcount: {
        ...state.msgcount,
        [taskId]: {
          ...(state.msgcount[taskId] ?? {}),
          [stepId]: 0,
        },
      },
    }));
  },
  hydrate: async () => {
    await listen<MsgData>('workflow-step-message', ({ payload }) => {
      void get().appendMessage(payload);
    });

    const { msgdata, counts } =
      await readLatestMessagesGroupByStep(OLDER_PAGE_SIZE);

    set({
      msgdata,
      msgcount: counts,
    });
  },
}));

void useMsgStore.getState().hydrate();
