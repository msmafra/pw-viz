use crate::pipewire_impl::PortType;

#[derive(Debug)]
pub struct Port {
    parent_pw_node_id: u32,
    id: u32,
    name: String,
    port_type: PortType,
}
impl Port {
    pub fn new(id: u32, parent_pw_node_id: u32, name: String, port_type: PortType) -> Self {
        Self {
            id,
            parent_pw_node_id,
            name,
            port_type,
        }
    }
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn parent_pw_node_id(&self) -> u32 {
        self.parent_pw_node_id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn port_type(&self) -> PortType {
        self.port_type
    }
}
