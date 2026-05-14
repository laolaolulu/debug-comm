import zhCN from "antd/locale/zh_CN";
import enUS from "antd/locale/en_US";
import zhCNMessages from "./locales/zh-CN.json";
import enUSMessages from "./locales/en-US.json";

export const langConfigMap: Record<string, any> = {
  "zh-CN": {
    lang: "zh-CN",
    label: "简体中文",
    icon: "🇨🇳",
    title: "语言",
    antd: zhCN,
    locale: zhCNMessages,
  },
  "en-US": {
    lang: "en-US",
    label: "English",
    icon: "🇺🇸",
    title: "Language",
    antd: enUS,
    locale: enUSMessages,
  },
};
