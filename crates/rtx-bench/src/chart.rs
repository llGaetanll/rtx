use std::collections::HashMap;

use svg::Document;
use svg::node::element::Group;
use svg::node::element::Line;
use svg::node::element::Polyline;
use svg::node::element::Rectangle;
use svg::node::element::Text;

/// Color palette for chart lines.
const COLORS: &[&str] = &[
    "#e41a1c", // red
    "#377eb8", // blue
    "#4daf4a", // green
    "#984ea3", // purple
    "#ff7f00", // orange
    "#a65628", // brown
    "#f781bf", // pink
    "#999999", // gray
];

/// Chart dimensions and layout.
const CHART_WIDTH: f64 = 800.0;
const CHART_HEIGHT: f64 = 300.0;
const CHART_PADDING: f64 = 60.0;
const CHART_SPACING: f64 = 40.0;
const LEGEND_LINE_HEIGHT: f64 = 20.0;

/// Generate an SVG group for a single benchmark chart.
fn generate_benchmark_chart(name: &str, data: &HashMap<String, Vec<u64>>, y_offset: f64) -> Group {
    let mut group = Group::new();

    // Sort SHAs for consistent ordering
    let mut shas: Vec<_> = data.keys().collect();
    shas.sort();

    // Find bounds
    let max_frames = data.values().map(|v| v.len()).max().unwrap_or(0);
    let max_time = data
        .values()
        .flat_map(|v| v.iter())
        .copied()
        .max()
        .unwrap_or(1);

    if max_frames == 0 {
        return group;
    }

    let plot_width = CHART_WIDTH - 2.0 * CHART_PADDING;
    let plot_height = CHART_HEIGHT - 2.0 * CHART_PADDING;
    let plot_x = CHART_PADDING;
    let plot_y = y_offset + CHART_PADDING;

    // Chart title
    let title = Text::new(name)
        .set("x", plot_x)
        .set("y", y_offset + 20.0)
        .set("font-family", "monospace")
        .set("font-size", 14)
        .set("font-weight", "bold");
    group = group.add(title);

    // X axis
    let x_axis = Line::new()
        .set("x1", plot_x)
        .set("y1", plot_y + plot_height)
        .set("x2", plot_x + plot_width)
        .set("y2", plot_y + plot_height)
        .set("stroke", "#aaa")
        .set("stroke-width", 1);
    group = group.add(x_axis);

    // Y axis
    let y_axis = Line::new()
        .set("x1", plot_x)
        .set("y1", plot_y)
        .set("x2", plot_x)
        .set("y2", plot_y + plot_height)
        .set("stroke", "#aaa")
        .set("stroke-width", 1);
    group = group.add(y_axis);

    // Draw data lines
    for (i, sha) in shas.iter().enumerate() {
        let color = COLORS[i % COLORS.len()];
        let frame_times = &data[*sha];

        if frame_times.is_empty() {
            continue;
        }

        let points: Vec<(f64, f64)> = frame_times
            .iter()
            .enumerate()
            .map(|(frame, &time_us)| {
                let x = plot_x + (frame as f64 / max_frames as f64) * plot_width;
                let y = plot_y + plot_height - (time_us as f64 / max_time as f64) * plot_height;
                (x, y)
            })
            .collect();

        let points_str: String = points
            .iter()
            .map(|(x, y)| format!("{:.1},{:.1}", x, y))
            .collect::<Vec<_>>()
            .join(" ");

        let polyline = Polyline::new()
            .set("points", points_str)
            .set("fill", "none")
            .set("stroke", color)
            .set("stroke-width", 1.5);
        group = group.add(polyline);
    }

    // Draw legend
    let legend_x = plot_x + plot_width + 10.0;
    let legend_y = plot_y;

    for (i, sha) in shas.iter().enumerate() {
        let color = COLORS[i % COLORS.len()];
        let y = legend_y + (i as f64) * LEGEND_LINE_HEIGHT;

        // Color swatch
        let swatch = Rectangle::new()
            .set("x", legend_x)
            .set("y", y)
            .set("width", 12)
            .set("height", 12)
            .set("fill", color);
        group = group.add(swatch);

        // SHA label
        let label = Text::new(*sha)
            .set("x", legend_x + 16.0)
            .set("y", y + 10.0)
            .set("font-family", "monospace")
            .set("font-size", 12);
        group = group.add(label);
    }

    group
}

/// Generate a complete SVG with charts for all benchmarks.
///
/// Takes the output of `load_all_benchmarks`: benchmark_name -> git_sha -> frame_times
pub fn generate_svg(data: &HashMap<String, HashMap<String, Vec<u64>>>) -> String {
    // Sort benchmark names for consistent ordering
    let mut names: Vec<_> = data.keys().collect();
    names.sort();

    // Calculate total dimensions
    let total_height = names.len() as f64 * (CHART_HEIGHT + CHART_SPACING);
    let total_width = CHART_WIDTH + 120.0; // Extra space for legend

    // Create document
    let mut document =
        Document::new().set("viewBox", (0, 0, total_width as i32, total_height as i32));

    // Background
    let background = Rectangle::new()
        .set("width", "100%")
        .set("height", "100%")
        .set("fill", "#fafafa");
    document = document.add(background);

    // Generate each chart
    for (i, name) in names.iter().enumerate() {
        let y_offset = i as f64 * (CHART_HEIGHT + CHART_SPACING);
        let chart_data = &data[*name];
        let chart_group = generate_benchmark_chart(name, chart_data, y_offset);
        document = document.add(chart_group);
    }

    document.to_string()
}
