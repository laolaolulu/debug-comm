import { Store } from "@tauri-apps/plugin-store";

// Tauri Store 文件名。实际文件会保存在应用数据目录下，而不是浏览器 localStorage。
const STORE_FILE = "debug-com.json";

let storePromise: Promise<Store> | undefined;

// Store.load 是异步的；这里缓存 Promise，避免多个 model 重复加载同一个 Store。
export const getAppStore = () => {
  storePromise ??= Store.load(STORE_FILE);
  return storePromise;
};

// 读取 Store 中的指定 key；不存在或读取失败时返回 fallback，保证界面可以继续初始化。
export async function getStoreValue<T>(key: string, fallback: T): Promise<T> {
  try {
    const store = await getAppStore();
    const value = await store.get<T>(key);
    return value ?? fallback;
  } catch (error) {
    console.error(`读取 Store 失败：${key}`, error);
    return fallback;
  }
}

// 写入 Store。plugin-store 默认会自动保存，这里仍显式 save，让关键状态更快落盘。
export async function setStoreValue(key: string, value: unknown) {
  try {
    const store = await getAppStore();
    await store.set(key, value);
    await store.save();
  } catch (error) {
    console.error(`写入 Store 失败：${key}`, error);
  }
}
