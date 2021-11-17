use std::{collections::HashMap, hash::Hash};

use crate::pipewire_impl::MediaType;

use super::{id::Id, port::Port};

#[derive(Debug)]
pub struct Node {
    id: Id,
    name: String,
    pw_nodes: HashMap<u32, PwNode>,
    pub(super) position: Option<egui::Pos2>,
}

impl Node {
    pub fn new(id: Id, name: String) -> Self {
        Self {
            id,
            name,
            pw_nodes: HashMap::new(),
            position: None,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn id(&self) -> Id {
        self.id
    }

    pub(super) fn add_pw_node(&mut self, id: u32, media_type: Option<MediaType>) {
        self.pw_nodes.insert(id, PwNode {
            id,
            media_type,
            ports: HashMap::new()
        });
    }
    //Use pooling
    pub(super) fn remove_pw_node(&mut self, id: u32) -> bool {
        self.pw_nodes.remove(&id);

        self.pw_nodes.is_empty()
    }

    #[inline]
    pub fn get_pw_node(&mut self, id: u32) -> Option<&mut PwNode> {
        self.pw_nodes.get_mut(&id)
    }

    pub fn add_port(&mut self, node_id: u32, port: Port) {
        let pw_node = self.get_pw_node(node_id);

        pw_node
        .expect(&format!("Couldn't find pipewire node with id {}", port.id()))
        .ports.insert(port.id(), port);
    }
    pub fn remove_port(&mut self, node_id: u32, port_id: u32) {
        if let Some(pw_node) = self.pw_nodes.get_mut(&node_id) {
            pw_node.ports.remove(&port_id);
        }
        else {
            log::error!("Pipewire node with id: {} was never added", node_id);
        }
    }
}

#[derive(Debug)]
struct PwNode {
    id: u32, //Pipewire id of the node
    media_type: Option<MediaType>,
    ports: HashMap<u32, Port>
}

impl PwNode {
}