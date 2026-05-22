import { create } from "zustand";
import { getStoreValue, setStoreValue } from "../appStore";

// Store 中保存界面语言的 key。
const LOCALE_STORAGE_KEY = "locale";

// 国际化状态。
export const useLocaleStore = create<{
  locale: string;
  setLocale: (locale: string) => void;
  hydrate: () => Promise<void>;
}>((set) => ({
  // Store 是异步读取，先使用默认中文，读取完成后 hydrate 会覆盖为持久化值。
  locale: "zh-CN",

  // 从 Tauri Store 恢复用户选择。
  hydrate: async () => {
    const locale = await getStoreValue<string>(LOCALE_STORAGE_KEY, "zh-CN");
    set({ locale });
  },

  // 更新语言后写入 Tauri Store，刷新页面仍保持用户选择。
  setLocale: (locale) => {
    set({ locale });
    void setStoreValue(LOCALE_STORAGE_KEY, locale);
  },
}));

void useLocaleStore.getState().hydrate();
