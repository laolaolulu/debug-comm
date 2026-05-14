# 数据持久化全局共享

选择`zustand`实现`React Context`功能，最简化设计，不用任何多余的扩展设计，字符串不要设计类型管理

## `activeTab` 菜单栏功能页切换

- 软件启动首次默认展示`工作台`页面
- 点击菜单栏项展示对应的页面
- 非菜单栏也可切换，所以全局共享State设计

## `locale` 国际化语言切换

- 当前使用语言持久化到`tauri`的`store` 
- 软件启动首次读取持久化数据，没有就默认`zh-CN`
- 配合`react-intl`实现中英文切换功能

## `workrun` 当前执行的任务集合

- 软件启动数据从后台`workflow.WORKFLOW_INSTANCES`获取
- 数据类型为`string[]`的任务id
- 任务启动添加id，任务结束移除id
- 目的用来管理工作台启动暂停按钮状态
- 还需要管理`SelectWork`选项下拉任务列表项显示任务状态标识

## `workflow` 任务切换管理等功能

### `workflows`所有任务`json`对象

- 所有任务持久化到`tauri`的`store` 

- 软件启动首次读取持久化数据，没有就创建：

  ```json
  {
   id: String(Date.now()),
   name: "New Blank",
   nodes: [],
   edges: []
  }
  ```
- `SelectWork`选项下拉任务列表`{value:id,label:name}`
- `workflows`有发生改变需要更新持久化数据

### `select`当前选中的任务`json`对象

- 选中的任务id持久化到`tauri`的`store` 
- 首次启动读取持久化id去`workflows`查询到任务`json`对象，没有读取到id就加载`workflows[0]`，所以这个state需要订阅`workflows`变化

- 主要实现`SelectWork`任务切换功能
- 保存功能就是把`select`的内容修改进`workflows`

### `isChange`当前选中的任务是否发生变化

- 订阅`select`与`workflows`变化，通过`select.id`查询`workflows`项对比`json`对象是否一致:一致false,不一致true
- 保存按钮如果没有改变不可点击
- `activeTab，SelectWork`切换时先检查，如果`isChange==true`就需要弹出提示先要保存后再操作