import { ReactFlowProvider } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import './index.css';
import Header from './components/Header';
import StepList from './components/StepList';
import StepFlow from './components/StepFlow';
import StepPar from './components/StepPar';
import { Flex, Splitter } from 'antd';
import { defineMessages } from 'react-intl';

export const nodeType = defineMessages({
  DisInputStep: {
    id: 'node.type.DisInputStep',
    defaultMessage: '发送数据窗口',
  },
  DisOutputStep: {
    id: 'node.type.DisOutputStep',
    defaultMessage: '接收数据窗口',
  },
  SerialPortStep: {
    id: 'node.type.SerialPortStep',
    defaultMessage: '串口通信',
  },
  TcpClientStep: {
    id: 'node.type.TcpClientStep',
    defaultMessage: 'TCP 客户端',
  },
  TcpServerStep: {
    id: 'node.type.TcpServerStep',
    defaultMessage: 'TCP 服务端',
  },
  JavaScriptStep: {
    id: 'node.type.JavaScriptStep',
    defaultMessage: 'JS 自动化脚本',
  },
});

export default () => {
  return (
    <Flex vertical>
      <Header />
      <ReactFlowProvider>
        <Splitter>
          <Splitter.Panel
            defaultSize={200}
            min={150}
            max={300}
            style={{ overflow: 'visible', zIndex: 10 }}
          >
            <StepList />
          </Splitter.Panel>
          <Splitter.Panel style={{ height: 'calc(100vh - 100px)' }}>
            <StepFlow />
          </Splitter.Panel>
          <Splitter.Panel defaultSize={200} min={150} max={300}>
            <StepPar />
          </Splitter.Panel>
        </Splitter>
      </ReactFlowProvider>
    </Flex>
  );
};
