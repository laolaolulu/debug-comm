use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// 消息类型枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MsgType {
    /// 消息向上发布。
    Up = 1,
    /// 消息向下发布。
    Down = 2,
}

/// 步骤消息体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepMsg<T> {
    /// 消息来自哪个步骤。
    pub step_id: String,
    /// 消息类型。
    pub action: MsgType,
    /// 消息数据。
    pub msg: T,
}

/// 工作流节点坐标。
/// 对齐前端 ReactFlow node.position 结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNodePosition {
    /// 节点在画布中的横坐标。
    pub x: f64,
    /// 节点在画布中的纵坐标。
    pub y: f64,
}

/// 工作流节点数据。
/// 对齐当前文档里的 name/description 设计，同时通过 extra 保留前端扩展字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNodeData {
    /// 节点显示名称，前端可修改。
    #[serde(default)]
    pub name: String,
    /// 节点说明，前端可修改。
    #[serde(default)]
    pub description: String,
    /// 节点参数表单定义。
    /// 结构直接对齐前端 ProFormColumnsType[]，可直接交给 BetaSchemaForm 渲染。
    #[serde(default)]
    pub columns: Vec<Value>,
    /// 保留节点 data 下的其他扩展字段。
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl WorkflowNodeData {
    /// 将当前节点 data 反序列化为具体步骤自己的参数结构。
    pub fn parse<T>(&self) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let mut map = serde_json::Map::<String, Value>::new();

        // name/description 是所有节点的公共字段，先放入参数对象。
        map.insert("name".to_string(), Value::String(self.name.clone()));
        map.insert(
            "description".to_string(),
            Value::String(self.description.clone()),
        );

        Self::collect_column_initial_values(&self.columns, &mut map);

        for (key, value) in &self.extra {
            map.insert(key.clone(), value.clone());
        }

        serde_json::from_value(Value::Object(map)).map_err(|err| err.to_string())
    }

    /// 递归提取表单定义中的 initialValue，组装成步骤参数对象。
    fn collect_column_initial_values(
        columns: &[Value],
        values: &mut serde_json::Map<String, Value>,
    ) {
        for column in columns {
            let Some(column_obj) = column.as_object() else {
                continue;
            };

            if let Some(data_index) = column_obj.get("dataIndex").and_then(Value::as_str) {
                if let Some(initial_value) = column_obj.get("initialValue") {
                    values.insert(data_index.to_string(), initial_value.clone());
                }
            }

            if let Some(children) = column_obj.get("columns").and_then(Value::as_array) {
                Self::collect_column_initial_values(children, values);
            }
        }
    }
}

/// 将通用 JSON 消息体转换为底层通信步骤使用的 byte[]。
///
/// 约定：
/// - 数字数组按 byte[] 写入。
/// - 其他 JSON 值一律拒绝，由调用方明确处理错误。
pub fn value_to_bytes(value: &Value) -> Result<Vec<u8>, String> {
    let Value::Array(items) = value else {
        return Err("message must be a byte array".to_string());
    };

    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let Some(value) = item.as_u64() else {
                return Err(format!("message byte at index {index} is not an integer"));
            };
            u8::try_from(value)
                .map_err(|_| format!("message byte out of range at index {index}: {value}"))
        })
        .collect()
}

/// 解析 16 进制结束符配置，例如 "0A0D" 或 "0A 0D"。
/// 返回 None 表示未配置结束符，通信步骤会按单次读取结果直接发布。
pub fn parse_hex_end_flag(value: Option<&str>) -> Result<Option<Vec<u8>>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let normalized = raw
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-' && *ch != ',')
        .collect::<String>();

    if normalized.is_empty() {
        return Ok(None);
    }
    if normalized.len() % 2 != 0 {
        return Err(format!("invalid hex end_flag length: {raw}"));
    }

    let mut bytes = Vec::with_capacity(normalized.len() / 2);
    let chars = normalized.as_bytes();
    for index in (0..chars.len()).step_by(2) {
        let part = std::str::from_utf8(&chars[index..index + 2]).map_err(|err| err.to_string())?;
        let byte = u8::from_str_radix(part, 16)
            .map_err(|err| format!("invalid hex end_flag byte `{part}`: {err}"))?;
        bytes.push(byte);
    }

    Ok(Some(bytes))
}

/// 在 buffer 中查找 needle 第一次出现的位置。
/// 标准库目前没有稳定的切片子串查找，这里保留一个很小的工具函数供拆包使用。
pub fn find_bytes(buffer: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }

    buffer
        .windows(needle.len())
        .position(|window| window == needle)
}

/// 步骤清单项。
/// 前端 StepList 可直接通过该结构展示步骤列表，并在拖拽创建节点时使用默认 data。
#[derive(Debug, Clone, Serialize)]
pub struct StepManifest {
    /// 步骤类型，对应 node.type。
    #[serde(rename = "type")]
    pub r#type: &'static str,
    /// 前端显示名称。
    pub data: StepManifestData,
}

/// 步骤清单默认节点 data。
#[derive(Debug, Clone, Serialize)]
pub struct StepManifestData {
    /// 前端显示名称。
    pub name: &'static str,
    /// 步骤说明。
    pub description: &'static str,

    /// 新建该步骤节点时默认写入的 data。
    pub columns: Vec<Value>,
}

/// 工作流定义结构。
/// 对齐前端工作流设计器传入的 JSON 结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// 工作流唯一标识，对应前端设计器中的 id。
    pub id: String,
    /// 工作流名称，对应前端设计器中的 name。
    pub name: String,
    /// 工作流描述，对应前端设计器中的 description。
    #[serde(default)]
    pub description: Option<String>,
    /// 节点列表，对应 ReactFlow 的 nodes。
    #[serde(default)]
    pub nodes: Vec<WorkflowNode>,
    /// 连线列表，对应 ReactFlow 的 edges。
    #[serde(default)]
    pub edges: Vec<WorkflowEdge>,
}

/// 工作流节点结构。
/// 对齐前端设计器和 ReactFlow 当前实际使用的节点字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    /// 节点唯一标识，对应前端 node.id。
    pub id: String,
    /// 节点类型，对应前端 node.type，例如 input/default/output。
    pub r#type: String,
    /// 节点位置，对应前端 node.position。
    pub position: WorkflowNodePosition,
    /// 节点业务数据，对应前端 node.data。
    pub data: WorkflowNodeData,
    /// 节点是否被选中。
    #[serde(default)]
    pub selected: bool,
    /// 保留节点其他未显式声明的字段，避免丢失 ReactFlow 扩展属性。
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
/// 工作流连线结构。
/// 对齐前端 ReactFlow edge 的核心字段，并保留其余扩展属性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    /// 连线唯一标识。
    #[serde(default)]
    pub id: Option<String>,
    /// 上游步骤 id，对应前端 ReactFlow edge.source。
    pub source: String,
    /// 下游步骤 id，对应前端 ReactFlow edge.target。
    pub target: String,
    /// 保留前端 edge 其他未显式声明的字段，避免丢失扩展信息。
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
