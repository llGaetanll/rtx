use std::collections::HashMap;
use std::fmt::Write;

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

/// Generate an SVG chart for a single benchmark.
fn generate_benchmark_chart(name: &str, data: &HashMap<String, Vec<u64>>, y_offset: f64) -> String {
    let mut svg = String::new();

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
        return svg;
    }

    let plot_width = CHART_WIDTH - 2.0 * CHART_PADDING;
    let plot_height = CHART_HEIGHT - 2.0 * CHART_PADDING;
    let plot_x = CHART_PADDING;
    let plot_y = y_offset + CHART_PADDING;

    // Chart title
    let _ = write!(
        svg,
        r#"<text x="{}" y="{}" font-family="monospace" font-size="14" font-weight="bold">{}</text>"#,
        plot_x,
        y_offset + 20.0,
        name
    );

    // Draw axes
    let axis_color = "#333";
    let _ = write!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
        plot_x,
        plot_y + plot_height,
        plot_x + plot_width,
        plot_y + plot_height,
        axis_color
    );
    let _ = write!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
        plot_x,
        plot_y,
        plot_x,
        plot_y + plot_height,
        axis_color
    );

    // Draw data lines
    for (i, sha) in shas.iter().enumerate() {
        let color = COLORS[i % COLORS.len()];
        let frame_times = &data[*sha];

        if frame_times.is_empty() {
            continue;
        }

        let mut points = String::new();
        for (frame, &time_us) in frame_times.iter().enumerate() {
            let x = plot_x + (frame as f64 / max_frames as f64) * plot_width;
            let y = plot_y + plot_height - (time_us as f64 / max_time as f64) * plot_height;
            if points.is_empty() {
                let _ = write!(points, "{:.1},{:.1}", x, y);
            } else {
                let _ = write!(points, " {:.1},{:.1}", x, y);
            }
        }

        let _ = write!(
            svg,
            r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="1.5"/>"#,
            points, color
        );
    }

    // Draw legend
    let legend_x = plot_x + plot_width + 10.0;
    let legend_y = plot_y;
    for (i, sha) in shas.iter().enumerate() {
        let color = COLORS[i % COLORS.len()];
        let y = legend_y + (i as f64) * LEGEND_LINE_HEIGHT;

        // Color swatch
        let _ = write!(
            svg,
            r#"<rect x="{}" y="{}" width="12" height="12" fill="{}"/>"#,
            legend_x, y, color
        );

        // SHA label
        let _ = write!(
            svg,
            r#"<text x="{}" y="{}" font-family="monospace" font-size="12">{}</text>"#,
            legend_x + 16.0,
            y + 10.0,
            sha
        );
    }

    svg
}

/// Generate a complete SVG with charts for all benchmarks.
///
/// Takes the output of `load_all_benchmarks`: benchmark_name -> git_sha -> frame_times
pub fn generate_svg(data: &HashMap<String, HashMap<String, Vec<u64>>>) -> String {
    let mut svg = String::new();

    // Sort benchmark names for consistent ordering
    let mut names: Vec<_> = data.keys().collect();
    names.sort();

    // Calculate total height
    let total_height = names.len() as f64 * (CHART_HEIGHT + CHART_SPACING);
    let total_width = CHART_WIDTH + 120.0; // Extra space for legend

    // SVG header
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}">"#,
        total_width, total_height
    );

    // Background
    let bg_color = "#fafafa";
    let _ = write!(
        svg,
        r#"<rect width="100%" height="100%" fill="{}"/>"#,
        bg_color
    );

    // Generate each chart
    for (i, name) in names.iter().enumerate() {
        let y_offset = i as f64 * (CHART_HEIGHT + CHART_SPACING);
        let chart_data = &data[*name];
        svg.push_str(&generate_benchmark_chart(name, chart_data, y_offset));
    }

    svg.push_str("</svg>");
    svg
}
