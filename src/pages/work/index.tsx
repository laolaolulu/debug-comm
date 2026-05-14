import { Splitter } from 'antd';
import Header from './components/Header';
import DisOutput from './components/DisOutput';
import DisInput from './components/DisInput';

export default () => {
  return (
    <div>
      <Header />
      <Splitter vertical style={{ height: 'calc(100vh - 98px)' }}>
        <Splitter.Panel><DisOutput /></Splitter.Panel>
        <Splitter.Panel defaultSize='30%'  min={110}><DisInput /></Splitter.Panel>
      </Splitter>
    </div>
  );
};
