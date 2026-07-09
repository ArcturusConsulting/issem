use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Vda5050NodePosition {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub theta: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Vda5050Node {
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "nodePosition")]
    pub node_position: Option<Vda5050NodePosition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Vda5050Order {
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub nodes: Vec<Vda5050Node>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Vda5050Action {
    #[serde(rename = "actionId")]
    pub action_id: String,
    #[serde(rename = "actionType")]
    pub action_type: String,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Vda5050InstantActions {
    #[serde(rename = "headerId")]
    pub header_id: u64,
    pub timestamp: u64,
    pub version: String,
    pub actions: Vec<Vda5050Action>,
}