mod state;

use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::mpsc::Sender};

use pipewire::{
    link::LinkChangeMask,
    prelude::ReadableDict,
    registry::{GlobalObject, Registry},
    spa::ForeignDict,
    Context, Core, MainLoop,
};

use crate::ui::UiMessage;

use self::state::State;
pub enum PipewireMessage {
    NodeAdded {
        id: u32,
        name: String,
        media_type: Option<MediaType>,
    },
    PortAdded {
        node_name: String,
        node_id: u32,
        id: u32,
        name: String,
        port_type: PortType,
    },
    LinkAdded {
        id: u32,
        from_node_name: String,
        to_node_name: String,

        from_port: u32,
        to_port: u32,
    },
    LinkStateChanged {
        id: u32,
        active: bool,
    },
    NodeRemoved {
        name: String,
        id: u32,
    },
    PortRemoved {
        node_name: String,
        node_id: u32,
        id: u32,
    },
    LinkRemoved {
        id: u32,
    },
}

#[derive(Debug, Copy, Clone)]
pub enum MediaType {
    Audio,
    Video,
    Midi,
}

#[derive(Copy, Clone, Debug)]
pub enum PortType {
    Input,
    Output,
    Unknown,
}

type Proxies = HashMap<u32, ProxyLink>;

#[allow(dead_code)]
struct ProxyLink {
    proxy: pipewire::link::Link,
    listener: pipewire::link::LinkListener,
}
///Pipewire mainloop runs on a separate thread, and notifies the UI thread of any changes using a mpsc channel
/// thread_main is the entry point of this thread
pub fn thread_main(
    sender: Rc<Sender<PipewireMessage>>,
    receiver: pipewire::channel::Receiver<UiMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mainloop = MainLoop::new()?;
    let context = Context::new(&mainloop)?;
    let core = Rc::new(context.connect(None)?);

    let proxies = Rc::new(RefCell::new(Default::default()));
    let proxies_rm = proxies.clone();

    let registry = Rc::new(core.get_registry()?);
    let registry_clone = registry.clone();

    let sender = sender.clone();
    let sender_rm = sender.clone();

    let state = Rc::new(RefCell::new(State::new()));
    let state_rm = state.clone();
    let state_rm_link = state.clone();

    let _listener = registry
        .add_listener_local()
        //Called when a global object is added
        .global({
            move |global| match global.type_ {
                pipewire::types::ObjectType::Node => {
                    handle_node(global, &state, &sender);
                }
                pipewire::types::ObjectType::Link => {
                    handle_link(global, &state, &sender, &registry_clone, &proxies);
                }
                pipewire::types::ObjectType::Port => {
                    handle_port(global, &state, &sender);
                }
                _ => {}
            }
        })
        //Called when a global object is removed
        .global_remove(move |id| match state_rm.borrow_mut().remove(id) {
            Some(object) => {
                let message = match object {
                    state::GlobalObject::Node {name} => PipewireMessage::NodeRemoved { name, id },
                    state::GlobalObject::Link => PipewireMessage::LinkRemoved { id },
                    state::GlobalObject::Port { node_name, node_id, id } => {
                        PipewireMessage::PortRemoved { node_name, node_id, id }
                    }
                };
                sender_rm
                    .send(message)
                    .expect("Failed to send pipewire message");

                proxies_rm.borrow_mut().remove(&id);
            }
            None => {
                log::warn!("Object with id: {} was never registered\n", id);
            }
        })
        .register();

    //This thread also receives messages from the ui thread to update the pipewire graph
    //Messages are sent on a special pipewire channel which needs to be registered with the main loop
    let _receiver = receiver.attach(&mainloop, {
        let state = state_rm_link;
        let mainloop = mainloop.clone();

        move |message| match message {
            UiMessage::RemoveLink(link_id) => {
                remove_link(link_id, &state, &registry);
            }
            UiMessage::AddLink {
                from_node,
                to_node,
                from_port,
                to_port,
            } => add_link(from_port, to_port, from_node, to_node, &core),
            UiMessage::Exit => mainloop.quit(),
        }
    });

    mainloop.run();

    Ok(())
}

fn handle_node(
    node: &GlobalObject<ForeignDict>,
    state: &Rc<RefCell<State>>,
    sender: &Rc<Sender<PipewireMessage>>,
) {
    let props = node
        .props
        .as_ref()
        .expect("Node object doesn't have properties");

    let name = props
        .get("node.nick")
        .or_else(|| props.get("node.description"))
        .or_else(|| props.get("node.name"))
        .unwrap_or_default()
        .to_string();

    let media_type = props.get("media.class").and_then(|class| {
        if class.contains("Audio") {
            Some(MediaType::Audio)
        } else if class.contains("Video") {
            Some(MediaType::Video)
        } else if class.contains("Midi") {
            Some(MediaType::Midi)
        } else {
            None
        }
    });

    state.borrow_mut().add(node.id, state::GlobalObject::Node {name: name.clone()});

    sender
        .send(PipewireMessage::NodeAdded {
            id: node.id,
            name,
            media_type,
        })
        .expect("Failed to send pipewire message");
}

fn handle_link(
    link: &GlobalObject<ForeignDict>,
    state: &Rc<RefCell<State>>,
    sender: &Rc<Sender<PipewireMessage>>,
    registry: &Rc<Registry>,
    proxies: &Rc<RefCell<Proxies>>,
) {
    let proxy: pipewire::link::Link = registry.bind(link).expect("Failed to bind link proxy");

    let sender = sender.clone();
    let state = state.clone();

    let listener = proxy
        .add_listener_local()
        .info(move |info| {
            let id = info.id();

            let from_node = info.output_node_id();
            let from_port = info.output_port_id();
            let to_node = info.input_node_id();
            let to_port = info.input_port_id();

            let mut state = state.borrow_mut();

            let from_node_name = match state.get(from_node)
                                            .expect("Id wasn't registered") {
                                                state::GlobalObject::Node { name } => name.clone(),
                                                _=> unreachable!()
                                            };
            let to_node_name = match state.get(from_node)
                                            .expect("Id wasn't registered") {
                                                state::GlobalObject::Node { name } => name.clone(),
                                                _=> unreachable!()
                                            };
                                                

            if let Some(&state::GlobalObject::Link) = state.get(id) {
                if info.change_mask().contains(LinkChangeMask::STATE) {
                    sender
                        .send(PipewireMessage::LinkStateChanged { id, active: true })
                        .expect("Failed to send pipewire message");
                }
            } else {
                state.add(id, state::GlobalObject::Link);
                log::debug!("New pipewire link was added : {}", id);
                sender
                    .send(PipewireMessage::LinkAdded {
                        from_node_name,
                        to_node_name,
                        from_port,
                        to_port,
                        id,
                    })
                    .expect("Failed to send pipewire message");
            }
        })
        .register();

    proxies
        .borrow_mut()
        .insert(link.id, ProxyLink { proxy, listener });
}
fn add_link(from_port: u32, to_port: u32, from_node: u32, to_node: u32, core: &Rc<Core>) {
    core.create_object::<pipewire::link::Link, _>(
        "link-factory",
        &pipewire::properties! {
            "link.input.port" => to_port.to_string(),
            "link.output.port" => from_port.to_string(),

            "link.input.node" => to_node.to_string(),
            "link.output.node"=> from_node.to_string(),

            "object.linger" => "1"

        },
    )
    .expect("Failed to add new link");
}
fn remove_link(link_id: u32, state: &Rc<RefCell<State>>, registry: &Rc<Registry>) {
    let state = state.borrow_mut();

    if let Some(&state::GlobalObject::Link) = state.get(link_id) {
        match registry.destroy_global(link_id).into_result() {
            Ok(_) => {}
            Err(err) => log::error!("SPA error: {}", err),
        }
    } else {
        log::warn!("Tried to destroy unregistered object with id: {}", link_id);
    }
}

fn handle_port(
    port: &GlobalObject<ForeignDict>,
    state: &Rc<RefCell<State>>,
    sender: &Rc<Sender<PipewireMessage>>,
) {
    let props = port
        .props
        .as_ref()
        .expect("Port object doesn't have properties");

    let name = props.get("port.name").unwrap_or_default().to_string();

    let node_id = props
        .get("node.id")
        .expect("Port object doesn't have node.id property")
        .parse::<u32>()
        .expect("Couldn't parse node.id as u32");
    
    let mut state = state.borrow_mut();

    let node_name = match state.get(node_id)
    .expect(&format!("Node with id {} was never registered", node_id)) {
        state::GlobalObject::Node { name } => name,
        _=> {
            unreachable!()
        }
    }.clone();
    

    let port_type = match props.get("port.direction") {
        Some("in") => PortType::Input,
        Some("out") => PortType::Output,
        _ => PortType::Unknown,
    };

    state.add(
        port.id,
        state::GlobalObject::Port {
            node_name: node_name.clone(),
            node_id,
            id: port.id,
        },
    );

    sender
        .send(PipewireMessage::PortAdded {
            node_name: node_name,
            node_id,
            id: port.id,
            name,
            port_type,
        })
        .expect("Failed to send pipewire message");
}
