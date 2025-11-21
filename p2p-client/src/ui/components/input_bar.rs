use eframe::egui;

pub fn render(ui: &mut egui::Ui, input_text: &mut String) -> Option<String> {
    let mut send = false;
    ui.horizontal(|ui| {
        let response = ui.text_edit_singleline(input_text);
        if ui.button("Send").clicked() {
            send = true;
        }

        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            send = true;
        }
    });

    if send && !input_text.is_empty() {
        let message = input_text.clone();
        input_text.clear();
        return Some(message);
    }

    None
}
