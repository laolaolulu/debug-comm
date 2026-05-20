import {
  CheckOutlined,
  CloudDownloadOutlined,
  GlobalOutlined,
} from "@ant-design/icons";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { App, Button, Dropdown, MenuProps, Space, Typography } from "antd";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLocaleStore } from "../models/locale";
import { langConfigMap } from "../constants";
import { useIntl } from "react-intl";

const isTauriRuntime = () => "__TAURI_INTERNALS__" in window;

export default () => {
  const { locale, setLocale } = useLocaleStore();
  const intl = useIntl();
  const { message, modal } = App.useApp();
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const startupCheckedRef = useRef(false);

  const formatMessage = useCallback(
    (
      id: string,
      defaultMessage: string,
      values?: Record<string, string | number>,
    ) =>
      intl.formatMessage({ id, defaultMessage }, values),
    [intl],
  );

  const checkForUpdates = useCallback(
    async (manual = false) => {
      if (!isTauriRuntime() || checkingUpdate) {
        return;
      }

      const messageKey = "app-update";
      setCheckingUpdate(true);
      if (manual) {
        message.loading({
          key: messageKey,
          content: formatMessage("update.checking", "Checking for updates..."),
          duration: 0,
        });
      }

      try {
        const update = await check({ timeout: 30000 });

        if (!update) {
          if (manual) {
            message.success({
              key: messageKey,
              content: formatMessage("update.none", "You are already up to date"),
              duration: 2,
            });
          }
          return;
        }

        message.destroy(messageKey);
        modal.confirm({
          title: formatMessage("update.availableTitle", "Update available: {version}", {
            version: update.version,
          }),
          content: (
            <Space direction="vertical" size={4}>
              <Typography.Text>
                {formatMessage(
                  "update.availableContent",
                  "Download and install this update now?",
                )}
              </Typography.Text>
              {update.body ? (
                <Typography.Paragraph style={{ marginBottom: 0, whiteSpace: "pre-wrap" }}>
                  {update.body}
                </Typography.Paragraph>
              ) : null}
            </Space>
          ),
          okText: formatMessage("update.install", "Install update"),
          cancelText: formatMessage("update.later", "Later"),
          onOk: async () => {
            let downloaded = 0;
            let contentLength = 0;

            message.loading({
              key: messageKey,
              content: formatMessage("update.downloading", "Downloading update..."),
              duration: 0,
            });

            await update.downloadAndInstall((event) => {
              if (event.event === "Started") {
                contentLength = event.data.contentLength ?? 0;
                return;
              }

              if (event.event === "Progress") {
                downloaded += event.data.chunkLength;
                if (contentLength > 0) {
                  message.loading({
                    key: messageKey,
                    content: formatMessage(
                      "update.downloadingPercent",
                      "Downloading update {percent}%",
                      {
                        percent: Math.floor((downloaded / contentLength) * 100),
                      },
                    ),
                    duration: 0,
                  });
                }
                return;
              }

              if (event.event === "Finished") {
                message.loading({
                  key: messageKey,
                  content: formatMessage("update.installing", "Installing update..."),
                  duration: 0,
                });
              }
            });

            message.success({
              key: messageKey,
              content: formatMessage("update.installed", "Update installed, restarting..."),
              duration: 1,
            });
            await relaunch();
          },
        });
      } catch (error) {
        if (manual) {
          message.error({
            key: messageKey,
            content: formatMessage("update.checkFailed", "Update check failed: {error}", {
              error: error instanceof Error ? error.message : String(error),
            }),
            duration: 4,
          });
        }
        console.error("Failed to check for updates", error);
      } finally {
        setCheckingUpdate(false);
      }
    },
    [checkingUpdate, formatMessage, message, modal],
  );

  useEffect(() => {
    if (startupCheckedRef.current) {
      return;
    }

    startupCheckedRef.current = true;
    void checkForUpdates(false);
  }, [checkForUpdates]);

  const items: MenuProps["items"] = useMemo(
    () =>
      Object.values(langConfigMap).map((item) => ({
        key: item.lang,
        icon: <span>{item.icon}</span>,
        label: (
          <Space style={{ minWidth: 120, justifyContent: "space-between" }}>
            <span>{item.label}</span>
            {item.lang === locale ? <CheckOutlined /> : null}
          </Space>
        ),
        onClick: () => setLocale(item.lang),
      })),
    [locale],
  );

  return (
    <Space>
      <Button
        type="text"
        icon={<CloudDownloadOutlined />}
        loading={checkingUpdate}
        title={formatMessage("update.check", "Check for updates")}
        onClick={() => void checkForUpdates(true)}
        style={{
          height: 45,
          border: 0,
          borderRadius: 0,
          paddingInline: 14,
        }}
      />
      <Dropdown
        menu={{
          selectedKeys: [locale],
          items,
        }}
        placement="bottomRight"
      >
        <Button
          type="text"
          icon={<GlobalOutlined />}
          style={{
            height: 45,
            border: 0,
            borderRadius: 0,
            paddingInline: 14,
          }}
          //  title={langConfigMap[locale].title}
        >
          {langConfigMap[locale].title}
        </Button>
      </Dropdown>
    </Space>
  );
};
