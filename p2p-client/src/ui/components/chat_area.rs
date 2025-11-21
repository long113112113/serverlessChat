use eframe::egui;

use crate::common::ChatMessage;

pub fn render(ui: &mut egui::Ui, messages: &[ChatMessage]) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        for message in messages {
            ui.label(format!("{}: {}", message.sender, message.content));
        }
    });
}
