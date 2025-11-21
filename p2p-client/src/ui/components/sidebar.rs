use crate::ui::state::AppState;
use eframe::egui;

#[derive(Default)]
pub struct SidebarActions {
    pub connect_address: Option<String>,
    pub friend_peer_id: Option<String>,
}

pub fn render(ui: &mut egui::Ui, state: &mut AppState) -> SidebarActions {
    let mut actions = SidebarActions::default();

    ui.heading("Peers");
    ui.separator();

    // Manual connect section
    ui.label("Connect to Peer:");
    ui.text_edit_singleline(&mut state.peer_address_input);
    if ui.button("Connect").clicked() {
        if !state.peer_address_input.trim().is_empty() {
            let address = state.peer_address_input.trim().to_string();
            state.peer_address_input.clear();
            actions.connect_address = Some(address);
        }
    }

    ui.separator();
    ui.label("Friends (Peer IDs):");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.friend_input);
        if ui.button("Add").clicked() {
            if !state.friend_input.trim().is_empty() {
                actions.friend_peer_id = Some(state.friend_input.trim().to_string());
                state.friend_input.clear();
            }
        }
    });

    if state.friends.is_empty() {
        ui.label("No friends added");
    } else {
        for status in state.friend_statuses() {
            ui.horizontal(|ui| {
                let color = if status.online {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::GRAY
                };
                ui.colored_label(color, if status.online { "●" } else { "○" });
                ui.label(&status.peer_id[..16.min(status.peer_id.len())]);
                ui.label(egui::RichText::new(status.message.clone()).weak());
            });
        }
    }

    ui.separator();
    ui.label("Connected Peers:");

    if state.peers.is_empty() {
        ui.label("No peers discovered yet");
        return actions;
    }

    for peer_id in &state.peers {
        ui.horizontal(|ui| {
            // Hiển thị trạng thái online với màu xanh
            ui.colored_label(egui::Color32::GREEN, "●");

            // Hiển thị peer ID (rút ngắn)
            ui.label(&peer_id[..16.min(peer_id.len())]);

            // Hiển thị last seen nếu có
            if let Some(last_seen) = state.peer_last_seen.get(peer_id) {
                let now = chrono::Utc::now();
                let elapsed = now.signed_duration_since(*last_seen);
                let seconds = elapsed.num_milliseconds() as f64 / 1000.0;

                if seconds < 1.0 {
                    ui.label(egui::RichText::new("(just now)").weak());
                } else {
                    ui.label(egui::RichText::new(format!("({:.0}s)", seconds)).weak());
                }
            }
        });
    }

    actions
}
