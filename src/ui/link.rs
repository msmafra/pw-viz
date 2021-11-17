use super::id::Id;

#[derive(Debug)]
pub struct Link {
    pub id: u32,
    pub from_node: Id,
    pub to_node: Id,

    pub from_port: u32,
    pub to_port: u32,
    pub active: bool,
}
