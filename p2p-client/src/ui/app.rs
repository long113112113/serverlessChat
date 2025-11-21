use eframe::egui;
use tokio::sync::mpsc;

use crate::common::{NetworkCommand, NetworkEvent};

use super::components::{
    chat_area, debug_panel, input_bar,
    sidebar::{self, SidebarActions},
};
use super::state::AppState;

pub struct ChatApp {
    state: AppState,
    command_sender: mpsc::Sender<NetworkCommand>,
    event_receiver: mpsc::Receiver<NetworkEvent>,
}

impl ChatApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        command_sender: mpsc::Sender<NetworkCommand>,
        event_receiver: mpsc::Receiver<NetworkEvent>,
    ) -> Self {
        Self {
            state: AppState::new(),
            command_sender,
            event_receiver,
        }
    }

    fn handle_network_events(&mut self) {
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                NetworkEvent::MessageReceived(message) => self.state.push_message(message),
                NetworkEvent::HistorySynced(history) => self.state.push_history(history),
                NetworkEvent::PeerConnected(peer_id) => self.state.add_peer(peer_id),
                NetworkEvent::PeerDisconnected(peer_id) => self.state.remove_peer(&peer_id),
                NetworkEvent::FriendStatus(status) => self.state.upsert_friend_status(status),
            }
        }
    }

    fn send_command(&mut self, payload: String) {
        if let Err(err) = self
            .command_sender
            .try_send(NetworkCommand::SendMessage(payload))
        {
            log::warn!("Failed to send command to network: {err}");
        }
    }

    fn connect_to_peer(&mut self, address: String) {
        if let Err(err) = self
            .command_sender
            .try_send(NetworkCommand::ConnectToPeer { address })
        {
            log::warn!("Failed to send connect command to network: {err}");
        }
    }

    fn add_friend(&mut self, peer_id: String) {
        if let Err(err) = self
            .command_sender
            .try_send(NetworkCommand::AddFriend { peer_id })
        {
            log::warn!("Failed to send add-friend command: {err}");
        }
    }
}

impl eframe::App for ChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_network_events();

        egui::SidePanel::left("peer_sidebar")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                let actions: SidebarActions = sidebar::render(ui, &mut self.state);
                if let Some(address) = actions.connect_address {
                    self.connect_to_peer(address);
                }
                if let Some(peer_id) = actions.friend_peer_id {
                    self.add_friend(peer_id);
                }
            });

        egui::SidePanel::right("debug_panel")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                debug_panel::render(ui, &self.state);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Rust P2P Chat");
            ui.separator();
            chat_area::render(ui, &self.state.messages);

            ui.separator();
            if let Some(content) = input_bar::render(ui, &mut self.state.input_text) {
                self.send_command(content);
            }
        });

        ctx.request_repaint();
    }
}
