use crate::ui::state::AppState;
use eframe::egui;

pub fn render(ui: &mut egui::Ui, state: &AppState) {
    ui.heading("Peers");
    ui.separator();

    if state.peers.is_empty() {
        ui.label("No peers discovered yet");
        return;
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
}
