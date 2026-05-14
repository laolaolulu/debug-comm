import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export const useWorkrunStore = create<{
  runningIds: string[];
  hydrate: () => Promise<void>;
  addRunning: (id: string) => void;
  removeRunning: (id: string) => void;
}>((set) => ({
  runningIds: [],

  hydrate: async () => {
    try {
      const ids = await invoke<string[]>("get_workflow_ids");
      set({ runningIds: ids });
    } catch (error) {
      console.error("读取运行中工作流失败", error);
    }
  },

  addRunning: (id) =>
    set((state) => ({
      runningIds: state.runningIds.includes(id)
        ? state.runningIds
        : [...state.runningIds, id],
    })),

  removeRunning: (id) =>
    set((state) => ({
      runningIds: state.runningIds.filter((i) => i !== id),
    })),
}));

void useWorkrunStore.getState().hydrate();
