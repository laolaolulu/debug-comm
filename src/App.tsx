import "./App.css";
import {
  ClusterOutlined,
  DesktopOutlined,
} from "@ant-design/icons";
import {
  App,
  ConfigProvider,
  Layout,
  Menu,
} from "antd";
import Designer from "./pages/flow";
import Workbench from "./pages/work";
import { ActiveTab, useActiveTabStore } from "./models/activeTab";
import { useLocaleStore } from "./models/locale";
import { useWorkflowIsChange } from "./models/workflow";
import { FormattedMessage, IntlProvider } from "react-intl";
import RightContent from "./pages/RightContent";
import { JSX } from "react";
import { langConfigMap } from "./constants";
const { Header, Content } = Layout;

const tabs: Record<string, JSX.Element> = {
  workbench: <Workbench />,
  designer: <Designer />,
};

const isActiveTab = (key: string): key is ActiveTab => key in tabs;

function AppContent() {
  const { activeTab, setActiveTab } = useActiveTabStore();
  const isChange = useWorkflowIsChange();
  const { modal } = App.useApp();

  const handleTabChange = (nextActiveTab: ActiveTab) => {
    if (isChange) {
      modal.warning({
        content: (
          <FormattedMessage
            id="save.warning"
            defaultMessage="请先保存，或者放弃重置"
          />
        ),
      });
      return;
    }
    setActiveTab(nextActiveTab);
  };

  return (
    <Layout>
      <Header style={{ display: "flex", alignItems: "center" }}>
        <Menu
          theme="light"
          mode="horizontal"
          selectedKeys={[activeTab]}
          onClick={({ key }) => {
            if (isActiveTab(key) && key !== activeTab) {
              handleTabChange(key);
            }
          }}
          style={{ flex: 1, minWidth: 0 }}
          items={[
            {
              key: "workbench",
              icon: <DesktopOutlined />,
              label: (
                <FormattedMessage id="menu.workbench" defaultMessage="工作台" />
              ),
            },
            {
              key: "designer",
              icon: <ClusterOutlined />,
              label: (
                <FormattedMessage id="menu.designer" defaultMessage="设计器" />
              ),
            },
          ]}
        />
        <RightContent />
      </Header>
      <Content style={{ height: "calc(100vh - 45px)" }}>
        {tabs[activeTab]}
      </Content>
    </Layout>
  );
}

export default () => {
  const locale = useLocaleStore((state) => state.locale);
  return (
    <IntlProvider
      locale={locale}
      messages={langConfigMap[locale].locale}
    >
      <ConfigProvider
        locale={langConfigMap[locale].antd}
        theme={{
          components: {
            Layout: {
              headerPadding: 10,
              headerBg: "#fff",
              headerHeight: 45,
            },
          },
        }}
      >
        <App>
          <AppContent />
        </App>
      </ConfigProvider>
    </IntlProvider>
  );
};
