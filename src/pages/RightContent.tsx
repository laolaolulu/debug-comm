import { CheckOutlined, GlobalOutlined } from "@ant-design/icons";
import { Button, Dropdown, MenuProps, Space } from "antd";
import { useMemo } from "react";
import { useLocaleStore } from "../models/locale";
import { langConfigMap } from "../constants";

export default () => {
  const { locale, setLocale } = useLocaleStore();

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
