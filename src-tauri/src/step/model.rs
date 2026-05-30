use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// 消息类型。
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

/// 工作流节点结构。
#[derive(Deserialize)]
pub struct WorkflowNode {
    /// 节点唯一标识。
    pub id: String,
    /// 节点类型。
    #[serde(rename = "type")]
    pub r#type: String,
    /// 节点业务数据。
    pub data: WorkflowNodeData,
}

/// 工作流节点数据。
#[derive(Deserialize)]
pub struct WorkflowNodeData {
    /// 节点显示名称。
    pub name: String,
    /// 节点说明。
    pub description: String,
    /// 节点参数表单定义。
    pub columns: Vec<Value>,
    /// 节点具体参数。
    #[serde(flatten)]
    pub params: HashMap<String, Value>,
}

impl WorkflowNodeData {
    /// 按字段名获取节点参数，实际参数优先于表单初始值。
    pub fn value(&self, key: &str) -> Option<&Value> {
        self.params
            .get(key)
            .or_else(|| Self::column_initial_value(&self.columns, key))
    }

    /// 递归读取表单字段的 initialValue。
    fn column_initial_value<'a>(columns: &'a [Value], key: &str) -> Option<&'a Value> {
        for column in columns {
            let Some(column_obj) = column.as_object() else {
                continue;
            };

            if column_obj.get("dataIndex").and_then(Value::as_str) == Some(key) {
                return column_obj.get("initialValue");
            }

            if let Some(children) = column_obj.get("columns").and_then(Value::as_array) {
                if let Some(value) = Self::column_initial_value(children, key) {
                    return Some(value);
                }
            }
        }
        None
    }
}

/// 将通用 JSON 消息体转换为底层通信步骤使用的 byte[]。
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

/// 解析 16 进制结束符配置。
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
pub fn find_bytes(buffer: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }

    buffer
        .windows(needle.len())
        .position(|window| window == needle)
}

/// 步骤清单项。
#[derive(Serialize)]
pub struct StepManifest {
    /// 步骤类型。
    #[serde(rename = "type")]
    pub r#type: &'static str,
    /// 默认节点 data。
    pub data: StepManifestData,
}

/// 步骤清单默认节点 data。
#[derive(Serialize)]
pub struct StepManifestData {
    /// 前端显示名称。
    pub name: &'static str,
    /// 步骤说明。
    pub description: &'static str,
    /// 节点参数表单配置。
    pub columns: Vec<Value>,
}

/// 工作流定义结构。
#[derive(Deserialize)]
pub struct WorkflowDefinition {
    /// 工作流唯一标识。
    pub id: String,
    /// 工作流名称。
    pub name: String,
    /// 工作流描述。
    #[serde(default)]
    pub description: Option<String>,
    /// 节点列表。
    #[serde(default)]
    pub nodes: Vec<WorkflowNode>,
    /// 连线列表。
    #[serde(default)]
    pub edges: Vec<WorkflowEdge>,
}

/// 工作流连线结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    /// 连线唯一标识。
    #[serde(default)]
    pub id: Option<String>,
    /// 上游步骤 id。
    pub source: String,
    /// 下游步骤 id。
    pub target: String,
    /// 保留前端 edge 其他字段。
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
