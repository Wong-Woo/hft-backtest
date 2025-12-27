use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Legend, Corner, AxisHints};
use std::collections::VecDeque;

pub struct ChartRenderer;

impl ChartRenderer {
    /// Format time in seconds to human-readable format for x-axis
    fn format_time_axis(seconds: f64) -> String {
        let secs = seconds.abs();
        if secs >= 3600.0 {
            let hours = (secs / 3600.0).floor() as u64;
            let mins = ((secs % 3600.0) / 60.0).floor() as u64;
            format!("{}h{:02}m", hours, mins)
        } else if secs >= 60.0 {
            let mins = (secs / 60.0).floor() as u64;
            let s = (secs % 60.0).floor() as u64;
            format!("{}m{:02}s", mins, s)
        } else {
            format!("{:.0}s", secs)
        }
    }

    pub fn render_line_chart(
        ui: &mut egui::Ui,
        id: &str,
        title: &str,
        data: &VecDeque<(f64, f64)>,
        width: f32,
        color: egui::Color32,
        name: &str,
        show_zero_line: bool,
        baseline: Option<f64>,
    ) {
        ui.label(egui::RichText::new(title).strong().size(14.0));
        
        if data.is_empty() {
            ui.add_sized([width, 180.0], egui::Label::new("No data available"));
            return;
        }
        
        let points: PlotPoints = data.iter().map(|(t, v)| [*t, *v]).collect();
        
        // Custom x-axis formatter for time
        let x_axis = AxisHints::new_x()
            .label("Time")
            .formatter(|mark, _range| Self::format_time_axis(mark.value));

        Plot::new(id)
            .legend(Legend::default().position(Corner::LeftTop))
            .height(180.0)
            .width(width)
            .show_axes([true, true])
            .custom_x_axes(vec![x_axis])
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new(points).color(color).name(name).width(2.0));
                
                if let Some(baseline_val) = baseline {
                    if !data.is_empty() {
                        let start = data.front().unwrap().0;
                        let end = data.back().unwrap().0;
                        let baseline_pts: PlotPoints = vec![[start, baseline_val], [end, baseline_val]].into();
                        plot_ui.line(
                            Line::new(baseline_pts)
                                .color(egui::Color32::GRAY)
                                .name("Baseline")
                                .style(egui_plot::LineStyle::Dashed { length: 10.0 })
                        );
                    }
                }
                
                if show_zero_line && !data.is_empty() {
                    let start = data.front().unwrap().0;
                    let end = data.back().unwrap().0;
                    let zero_line: PlotPoints = vec![[start, 0.0], [end, 0.0]].into();
                    plot_ui.line(
                        Line::new(zero_line)
                            .color(egui::Color32::GRAY)
                            .style(egui_plot::LineStyle::Dashed { length: 10.0 })
                    );
                }
            });
    }
}
