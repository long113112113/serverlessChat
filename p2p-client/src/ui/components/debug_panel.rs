use eframe::egui;

use crate::ui::state::AppState;

pub fn render(ui: &mut egui::Ui, state: &AppState) {
    ui.heading("Debug Info");
    ui.separator();

    // Hiển thị số lượng peers hiện tại
    ui.horizontal(|ui| {
        ui.label("Active Peers:");
        ui.label(format!("{}", state.peers.len()));
    });

    ui.separator();

    // Hiển thị thông tin từng peer với last seen
    ui.label("Peer Status:");
    for peer_id in &state.peers {
        if let Some(last_seen) = state.peer_last_seen.get(peer_id) {
            let now = chrono::Utc::now();
            let elapsed = now.signed_duration_since(*last_seen);
            ui.horizontal(|ui| {
                ui.label(format!("✓ {}", &peer_id[..8]));
                ui.label(format!(
                    "Last seen: {:.1}s ago",
                    elapsed.num_milliseconds() as f64 / 1000.0
                ));
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(format!("✓ {}", &peer_id[..8]));
                ui.label("(connecting...)");
            });
        }
    }

    ui.separator();

    // Hiển thị disconnected peers với thời gian offline
    let disconnected_peers: Vec<_> = state
        .peer_last_seen
        .iter()
        .filter(|(peer_id, _)| !state.peers.contains(*peer_id))
        .collect();

    if !disconnected_peers.is_empty() {
        ui.label("Disconnected Peers:");
        for (peer_id, last_seen) in disconnected_peers {
            let now = chrono::Utc::now();
            let elapsed = now.signed_duration_since(*last_seen);
            ui.horizontal(|ui| {
                ui.label(format!("✗ {}", &peer_id[..8]));
                ui.label(format!(
                    "Offline: {:.1}s",
                    elapsed.num_milliseconds() as f64 / 1000.0
                ));
            });
        }
        ui.separator();
    }

    // Hiển thị log events gần đây
    ui.label("Recent Events:");
    egui::ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            for event in state.debug_events.iter().rev().take(20) {
                let time_str = event.timestamp.format("%H:%M:%S");
                let color = match event.event_type.as_str() {
                    "PEER_CONNECTED" => egui::Color32::GREEN,
                    "PEER_DISCONNECTED" => egui::Color32::RED,
                    "PEER_REFRESHED" => egui::Color32::YELLOW,
                    _ => egui::Color32::WHITE,
                };

                ui.horizontal(|ui| {
                    ui.colored_label(color, format!("[{}]", time_str));
                    ui.label(&event.message);
                });
            }
        });
}
