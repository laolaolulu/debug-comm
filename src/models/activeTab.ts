import { create } from "zustand";

// 顶部页签独立 store，避免 workflow store 混入纯 UI 状态。
export const useActiveTabStore = create<{
  activeTab: string;
  setActiveTab: (activeTab: string) => void;
}>((set) => ({
  // 默认进入工作台，避免刷新后停留在编辑状态带来误操作。
  activeTab: "workbench",

  // 更新当前页签，保留 React setState 的函数式更新写法。
  setActiveTab: (activeTab) => set(() => ({ activeTab })),
}));
