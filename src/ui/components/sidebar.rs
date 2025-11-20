use eframe::egui;

pub fn render(ui: &mut egui::Ui, peers: &[String]) {
    ui.heading("Peers");
    ui.separator();

    if peers.is_empty() {
        ui.label("No peers discovered yet");
        return;
    }

    for peer in peers {
        ui.label(peer);
    }
}
