import { transformFileAsync } from "@babel/core";
import fs from "node:fs/promises";
import path from "node:path";
import reactIntlPlugin from "babel-plugin-react-intl";

const rootDir = process.cwd();
const srcDir = path.join(rootDir, "src");
const localesDir = path.join(srcDir, "locales");
const messageFilesDir = path.join(localesDir, "messages");
const zhCNPath = path.join(localesDir, "zh-CN.json");
const enUSPath = path.join(localesDir, "en-US.json");

const sourceExts = new Set([".ts", ".tsx", ".js", ".jsx"]);

const enUSDefaults = {
  "工作台": "Workbench",
  "设计器": "Designer",
  "工作流节点": "Workflow Nodes",
  "{count} 个类型": "{count} types",
  "暂无节点": "No nodes",
  "导出": "Export",
  "选择导出文件夹": "Select Export Folder",
  "已导出：{name}": "Exported: {name}",
  "导出失败": "Export failed",
  "重置": "Reset",
  "删除模板": "Delete Template",
  "已删除：{name}": "Deleted: {name}",
  "删除警告": "Delete Warning",
  "是否删除标签模板【{name}】": "Delete template [{name}]?",
  "从空白页": "From Blank Page",
  "从当前模板": "From Current Template",
  "从文件导入": "Import From File",
  "新建": "New",
  "保存": "Save",
  "文件【{name}】非json格式,已忽略": "File [{name}] is not JSON and was ignored",
  "任务信息": "Task Info",
  "选中 {count} 个": "{count} selected",
  "任务编号": "Task ID",
  "任务名称": "Task Name",
  "任务描述": "Task Description",
  "节点名称": "Node Name",
  "节点描述": "Node Description",
  "删除": "Delete",
  "发送数据窗口": "Send Data",
  "接收数据窗口": "Receive Data",
  "串口通信": "Serial Port",
  "TCP 客户端": "TCP Client",
  "TCP 服务端": "TCP Server",
  "任务已启动：{workflowId}": "Workflow started: {workflowId}",
  "启动": "Start",
  "设置": "Settings",
  "请输入发送内容": "Enter content to send",
  "HEX 长度必须是偶数": "HEX length must be even",
  "非法 HEX 字节：{part}": "Invalid HEX byte: {part}",
  "已发送": "Sent",
  "发送": "Send",
  "暂无输入节点": "No input nodes",
  "清空": "Clear",
  "等待接收 HEX 数据": "Waiting for HEX data",
  "等待接收数据": "Waiting for data",
  "暂无接收节点": "No receive nodes",
};

async function listSourceFiles(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);

    if (entry.isDirectory()) {
      if (entry.name === "locales") {
        continue;
      }
      files.push(...(await listSourceFiles(fullPath)));
      continue;
    }

    if (entry.isFile() && sourceExts.has(path.extname(entry.name))) {
      files.push(fullPath);
    }
  }

  return files;
}

async function readJson(filePath) {
  try {
    return JSON.parse(await fs.readFile(filePath, "utf-8"));
  } catch {
    return {};
  }
}

function sortObject(value) {
  return Object.keys(value)
    .sort()
    .reduce((result, key) => {
      result[key] = value[key];
      return result;
    }, {});
}

function descriptorToMessageMap(descriptors) {
  return descriptors.reduce((result, descriptor) => {
    if (descriptor.id) {
      result[descriptor.id] = descriptor.defaultMessage ?? "";
    }
    return result;
  }, {});
}

async function extractFile(filePath) {
  const result = await transformFileAsync(filePath, {
    babelrc: false,
    configFile: false,
    code: false,
    ast: false,
    filename: filePath,
    parserOpts: {
      sourceType: "module",
      plugins: ["typescript", "jsx"],
    },
    plugins: [
      [
        reactIntlPlugin,
        {
          extractFromFormatMessageCall: true,
          extractSourceLocation: true,
        },
      ],
    ],
  });

  return result?.metadata?.["react-intl"]?.messages ?? [];
}

async function main() {
  await fs.mkdir(localesDir, { recursive: true });
  await fs.rm(messageFilesDir, { recursive: true, force: true });
  await fs.mkdir(messageFilesDir, { recursive: true });

  const files = await listSourceFiles(srcDir);
  const allDescriptors = [];

  for (const filePath of files) {
    const descriptors = await extractFile(filePath);
    if (descriptors.length === 0) {
      continue;
    }

    allDescriptors.push(...descriptors);

    const relativeName = path
      .relative(srcDir, filePath)
      .replace(/[\\/]/g, "__")
      .replace(/\.[^.]+$/, ".json");
    await fs.writeFile(
      path.join(messageFilesDir, relativeName),
      `${JSON.stringify(descriptors, null, 2)}\n`,
      "utf-8",
    );
  }

  const zhCNMessages = sortObject(descriptorToMessageMap(allDescriptors));
  const previousEnUS = await readJson(enUSPath);
  const enUSMessages = sortObject(
    Object.keys(zhCNMessages).reduce((result, key) => {
      result[key] =
        previousEnUS[key] || enUSDefaults[zhCNMessages[key]] || "";
      return result;
    }, {}),
  );

  await fs.writeFile(zhCNPath, `${JSON.stringify(zhCNMessages, null, 2)}\n`, "utf-8");
  await fs.writeFile(enUSPath, `${JSON.stringify(enUSMessages, null, 2)}\n`, "utf-8");

  console.log(`Extracted ${Object.keys(zhCNMessages).length} messages.`);
  console.log(`- ${path.relative(rootDir, zhCNPath)}`);
  console.log(`- ${path.relative(rootDir, enUSPath)}`);
  console.log(`- ${path.relative(rootDir, messageFilesDir)}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
