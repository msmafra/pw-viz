use std::collections::{HashMap, HashSet};

use egui_nodes::{LinkArgs, NodeArgs, NodeConstructor};

use crate::pipewire_impl::MediaType;

use super::{
    id::{Id, IdAllocator},
    link::Link,
    node::Node,
    port::Port,
    Theme,
};

///Represents changes to any links that might happend in the ui
///These changes are used to send updates to the pipewire thread
pub enum LinkUpdate {
    Created {
        from_port: u32,
        to_port: u32,

        from_node: u32,
        to_node: u32,
    },

    Removed(u32),
}
pub struct Graph {
    nodes_ctx: egui_nodes::Context,
    node_id_allocator: IdAllocator,
    nodes: HashMap<String, Node>, //Node name to Node
    links: HashMap<u32, Link>,    //Link id to Link
}

impl Graph {
    pub fn new() -> Self {
        //context.attribute_flag_push(egui_nodes::AttributeFlags::EnableLinkCreationOnSnap);
        //context.attribute_flag_push(egui_nodes::AttributeFlags::EnableLinkDetachWithDragClick);
        Self {
            nodes_ctx: egui_nodes::Context::default(),

            node_id_allocator: IdAllocator::new(),
            nodes: HashMap::new(),
            links: HashMap::new(),
        }
    }
    fn get_or_create_node(&mut self, name: String) -> &mut Node {
        self.nodes.entry(name.clone()).or_insert_with(|| {
            log::debug!("Created new ui node: {}", name);

            Node::new(self.node_id_allocator.allocate(), name)
        })
    }
    pub fn add_node(
        &mut self,
        name: String,
        id: u32,
        description: Option<String>,
        media_type: Option<MediaType>,
    ) {
        self.get_or_create_node(name)
            .add_pw_node(id, description, media_type)
    }
    pub fn remove_node(&mut self, name: &str, id: u32) {
        let mut remove_ui_node = false;

        if let Some(node) = self.nodes.get_mut(name) {
            remove_ui_node = node.remove_pw_node(id);
        } else {
            log::error!("Node with name: {} was not registered", name);
        }

        //If there are no more pw nodes remove the ui node
        if remove_ui_node {
            let removed_id = self.nodes.remove(name).expect("Node was never added").id();

            self.node_id_allocator.free(removed_id);
        }
    }
    pub fn add_port(&mut self, node_name: String, node_id: u32, port: Port) {
        self.get_or_create_node(node_name).add_port(node_id, port)
    }
    pub fn remove_port(&mut self, node_name: &str, node_id: u32, port_id: u32) {
        if let Some(node) = self.nodes.get_mut(node_name) {
            node.remove_port(node_id, port_id);
        } else {
            log::error!("Node with name: {} was not registered", node_name);
        }
    }
    pub fn add_link(
        &mut self,
        id: u32,
        from_node_name: String,
        to_node_name: String,
        from_port: u32,
        to_port: u32,
    ) {
        log::debug!(
            "{}.{}->{}.{}",
            from_node_name,
            from_port,
            to_node_name,
            to_port
        );

        let from_node = self
            .nodes
            .get(&from_node_name)
            .expect("Node with provided name doesn't exist")
            .id();

        let to_node = self
            .nodes
            .get(&to_node_name)
            .expect("Node with provided name doesn't exist")
            .id();
        log::debug!("{:?} {:?}", from_node, to_node);

        self.links.insert(
            id,
            Link {
                id,
                from_node,
                to_node,
                from_port,
                to_port,
                active: true,
            },
        );
    }
    pub fn remove_link(&mut self, id: u32) -> Option<Link> {
        let removed = self.links.remove(&id);

        match &removed {
            Some(link) => {
                log::debug!("{}-x-{}", link.from_port, link.to_port);
            }
            None => log::warn!("Link with id {} doesn't exist", id),
        }

        removed
    }
    #[allow(dead_code)]
    fn get_link(&self, id: u32) -> Option<&Link> {
        self.links.get(&id)
    }
    #[allow(dead_code)]
    fn get_link_mut(&mut self, id: u32) -> Option<&mut Link> {
        self.links.get_mut(&id)
    }
    fn topo_sort_(
        node_id: Id,
        visited: &mut HashSet<Id>,
        adj_list: &HashMap<Id, HashSet<Id>>,
        stack: &mut Vec<Id>,
    ) {
        visited.insert(node_id);

        for node_id in &adj_list[&node_id] {
            if !visited.contains(node_id) {
                Self::topo_sort_(*node_id, visited, adj_list, stack);
            }
        }

        stack.push(node_id);
    }
    //TODO: Handle stack overflows
    fn top_sort(&self) -> Vec<Id> {
        let mut stack = Vec::new();

        let mut visited = HashSet::new();

        let adj_list = self
            .nodes
            .values()
            .map(|node| {
                let adj = self
                    .links
                    .values()
                    .filter(|link| !link.is_self_link())
                    .filter(|link| link.from_node == node.id())
                    .map(|link| link.to_node)
                    .collect::<HashSet<Id>>();
                (node.id(), adj)
            })
            .collect::<HashMap<Id, _>>();

        for node in self.nodes.values() {
            if !visited.contains(&node.id()) {
                Self::topo_sort_(node.id(), &mut visited, &adj_list, &mut stack)
            }
        }

        stack.reverse();

        stack
    }
    pub fn draw(
        &mut self,
        ctx: &egui::CtxRef,
        ui: &mut egui::Ui,
        theme: &Theme,
    ) -> Option<LinkUpdate> {
        //Find the topologically sorted order of nodes in the graph
        //Nodes are currently laid out based on this order
        let order = self.top_sort();

        //Ctrl is used to trigger the debug view
        let debug_view = ctx.input().modifiers.ctrl;

        let mut ui_nodes = Vec::with_capacity(self.nodes.len());

        let mut prev_pos = egui::pos2(ui.available_width() / 4.0, ui.available_height() / 2.0);
        let mut padding = egui::pos2(75.0, 150.0);
        for node_id in order {
            let node = self
                .nodes
                .values()
                .find(|node| node.id() == node_id)
                .unwrap();

            let mut ui_node = NodeConstructor::new(
                node.id().as_usize(),
                NodeArgs {
                    titlebar: Some(theme.titlebar),
                    titlebar_hovered: Some(theme.titlebar_hovered),
                    titlebar_selected: Some(theme.titlebar_hovered),
                    ..Default::default()
                },
            );

            // if node.position.is_none() {

            //     //Horizontally shift each node to the right of the previous one
            //     //Also put it at a random point vertically
            //     ui_node.with_origin(egui::pos2(pos.x + padding, rand::random::<f32>() * ui.available_height()));

            // } else {
            //     ui_node.with_origin(egui::pos2(ui.available_width() / 4.0, rand::random::<f32>() * ui.available_height()));
            // }

            let node_position = node.position.unwrap_or_else(|| {
                padding.y *= -1.0;
                egui::pos2(prev_pos.x + padding.x, prev_pos.y + padding.y)
            });
            ui_node.with_origin(node_position);

            prev_pos = node_position;

            node.draw(&mut ui_node, theme, debug_view);

            ui_nodes.push(ui_node);
        }

        let links = self.links.values().map(|link| {
            (
                link.id as usize,
                link.from_port as usize,
                link.to_port as usize,
                LinkArgs::default(),
            )
        });

        self.nodes_ctx.show(ui_nodes, links, ui);

        for node in self.nodes.values_mut() {
            node.position = self
                .nodes_ctx
                .get_node_pos_screen_space(node.id().as_usize());
        }

        if let Some(link) = self.nodes_ctx.link_destroyed() {
            Some(LinkUpdate::Removed(link as u32))
        } else if let Some((from_port, from_node, to_port, to_node, _)) =
            self.nodes_ctx.link_created_node()
        {
            log::debug!(
                "Created new link:\nfrom_port {}, to_port {}, from_node {}, to_node {}",
                from_port,
                to_port,
                from_node,
                to_node
            );

            let from_port = from_port as u32;
            let to_port = to_port as u32;

            let from_node = from_node as u32;
            let to_node = to_node as u32;

            Some(LinkUpdate::Created {
                from_port,
                to_port,

                from_node,
                to_node,
            })
        } else {
            None
        }
    }

    // pub fn draw_old(
    //     &mut self,
    //     ctx: &egui::CtxRef,
    //     nodes_ctx: &mut egui_nodes::Context,
    //     ui: &mut egui::Ui,
    //     theme: &Theme,
    // ) -> Option<LinkUpdate> {
    //     let debug = cfg!(debug_assertions) && ctx.input().modifiers.ctrl;

    //     let ui_nodes = self
    //         .nodes
    //         .values_mut()
    //         .map(|node| {
    //             let mut ui_node = NodeConstructor::new(
    //                 node.id() as usize,
    //                 NodeArgs {
    //                     titlebar: Some(theme.titlebar),
    //                     titlebar_hovered: Some(theme.titlebar_hovered),
    //                     titlebar_selected: Some(theme.titlebar_hovered),
    //                     ..Default::default()
    //                 },
    //             );

    //             if node.newly_added {
    //                 ui_node.with_origin(egui::pos2(
    //                     rand::random::<f32>() * ui.available_height(),
    //                     rand::random::<f32>() * ui.available_height(),
    //                 ));
    //                 node.newly_added = false;
    //             }

    //             let media_type = node.media_type();
    //             let kind = match media_type {
    //                 Some(MediaType::Audio) => "🔉",
    //                 Some(MediaType::Video) => "💻",
    //                 Some(MediaType::Midi) => "🎹",
    //                 None => "",
    //             };

    //             let title = {
    //                 if debug {
    //                     format!("{}[{}]{}", node.name(), node.id(), kind)
    //                 } else {
    //                     format!("{} {}", node.name(), kind)
    //                 }
    //             };

    //             ui_node
    //                 .with_title(|ui| egui::Label::new(title).text_color(theme.text_color).ui(ui));

    //             Self::draw_ports(&mut ui_node, node, theme, debug);

    //             ui_node
    //         })
    //         .collect::<Vec<_>>();

    //     let links = self.links.values().map(|link| {
    //         (
    //             link.id as usize,
    //             link.from_port as usize,
    //             link.to_port as usize,
    //             LinkArgs::default(),
    //         )
    //     });

    //     nodes_ctx.show(ui_nodes, links, ui);

    //     if let Some(link) = nodes_ctx.link_destroyed() {
    //         Some(LinkUpdate::Removed(link as u32))
    //     } else if let Some((from_port, from_node, to_port, to_node, _)) =
    //         nodes_ctx.link_created_node()
    //     {
    //         log::debug!(
    //             "Created new link:\nfrom_port {}, to_port {}, from_node {}, to_node {}",
    //             from_port,
    //             to_port,
    //             from_node,
    //             to_node
    //         );

    //         let from_port = from_port as u32;
    //         let to_port = to_port as u32;

    //         let from_node = from_node as u32;
    //         let to_node = to_node as u32;

    //         Some(LinkUpdate::Created {
    //             from_port,
    //             to_port,

    //             from_node,
    //             to_node,
    //         })
    //     } else {
    //         None
    //     }
    // }
}
