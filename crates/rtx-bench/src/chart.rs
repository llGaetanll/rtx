use svg::Document;
use svg::node::element::ClipPath;
use svg::node::element::Definitions;
use svg::node::element::Group;
use svg::node::element::Line;
use svg::node::element::Polyline;
use svg::node::element::Rectangle;
use svg::node::element::Script;
use svg::node::element::Style;
use svg::node::element::Text;

use crate::BenchmarkData;

/// Base hues for benchmarks (HSL hue values 0-360).
const BASE_HUES: &[f64] = &[
    210.0, // blue
    120.0, // green
    30.0,  // orange
    280.0, // purple
    0.0,   // red
    180.0, // cyan
    330.0, // pink
    60.0,  // yellow
];

/// Chart dimensions and layout.
const CHART_WIDTH: f64 = 800.0;
const CHART_HEIGHT: f64 = 300.0;
const CHART_PADDING_LEFT: f64 = 80.0; // Extra space for y-axis labels
const CHART_PADDING_RIGHT: f64 = 60.0;
const CHART_PADDING_TOP: f64 = 60.0;
const CHART_PADDING_BOTTOM: f64 = 40.0;
const CHART_SPACING: f64 = 40.0;
const LEGEND_LINE_HEIGHT: f64 = 20.0;
const TARGET_TICKS: usize = 5;

/// Lightness range for color shades (oldest to newest).
const LIGHTNESS_MIN: f64 = 35.0; // darkest (newest)
const LIGHTNESS_MAX: f64 = 70.0; // lightest (oldest)
const SATURATION: f64 = 60.0;

/// Convert HSL to RGB hex string.
fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let s = s / 100.0;
    let l = l / 100.0;

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let r = ((r + m) * 255.0).round() as u8;
    let g = ((g + m) * 255.0).round() as u8;
    let b = ((b + m) * 255.0).round() as u8;

    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

/// Generate colors for a series of runs, from lightest (oldest) to darkest (newest).
fn generate_shade_colors(base_hue: f64, count: usize) -> Vec<String> {
    if count == 0 {
        return vec![];
    }
    if count == 1 {
        return vec![hsl_to_hex(base_hue, SATURATION, LIGHTNESS_MIN)];
    }

    (0..count)
        .map(|i| {
            // i=0 is oldest (lightest), i=count-1 is newest (darkest)
            let t = i as f64 / (count - 1) as f64;
            let lightness = LIGHTNESS_MAX - t * (LIGHTNESS_MAX - LIGHTNESS_MIN);
            hsl_to_hex(base_hue, SATURATION, lightness)
        })
        .collect()
}

/// Returns a list of "nice" tick mark positions for the given data range.
fn compute_nice_ticks(min: f64, max: f64) -> Vec<f64> {
    let range = max - min;
    if range == 0.0 {
        return vec![min];
    }

    // We want TARGET_TICKS intervals, which means TARGET_TICKS + 1 tick marks
    let raw_step = range / TARGET_TICKS as f64;

    // Find the magnitude (power of 10)
    let magnitude = 10_f64.powf(raw_step.log10().floor());

    // Normalize the step to be between 1 and 10
    let normalized_step = raw_step / magnitude;

    // Round to nearest "nice" number: 1, 2, 5, or 10
    let nice_step = if normalized_step <= 1.0 {
        1.0
    } else if normalized_step <= 2.0 {
        2.0
    } else if normalized_step <= 5.0 {
        5.0
    } else {
        10.0
    };

    let step = nice_step * magnitude;

    // Find nice start and end points (stay within data bounds)
    let nice_start = (min / step).floor() * step;
    let nice_end = (max / step).floor() * step;

    // Generate ticks
    let mut ticks = Vec::new();
    let mut current = nice_start;

    while current <= nice_end + f64::EPSILON {
        ticks.push(current);
        current += step;
    }

    ticks
}

/// Generate an SVG group for a single benchmark chart.
fn generate_benchmark_chart(benchmark: &BenchmarkData, base_hue: f64, y_offset: f64) -> Group {
    let mut group = Group::new();
    let chart_id = &benchmark.name;

    let runs = &benchmark.runs;
    if runs.is_empty() {
        return group;
    }

    // Find bounds - max_time for y-axis scaling
    let max_time = runs
        .iter()
        .flat_map(|r| r.frames.iter())
        .map(|f| f.time_us)
        .max()
        .unwrap_or(1);

    // Check we have frames
    let has_frames = runs.iter().any(|r| !r.frames.is_empty());
    if !has_frames {
        return group;
    }

    let plot_width = CHART_WIDTH - CHART_PADDING_LEFT - CHART_PADDING_RIGHT;
    let plot_height = CHART_HEIGHT - CHART_PADDING_TOP - CHART_PADDING_BOTTOM;
    let plot_x = CHART_PADDING_LEFT;
    let plot_y = y_offset + CHART_PADDING_TOP;

    // Create clip path for the plot area
    let clip_rect = Rectangle::new()
        .set("x", plot_x)
        .set("y", plot_y)
        .set("width", plot_width)
        .set("height", plot_height);
    let clip_path = ClipPath::new()
        .set("id", format!("clip-{}", chart_id))
        .add(clip_rect);
    let defs = Definitions::new().add(clip_path);
    group = group.add(defs);

    // Determine units and scale factor for display
    let (unit, scale) = if max_time >= 1_000_000 {
        ("s", 1_000_000.0)
    } else if max_time >= 1_000 {
        ("ms", 1_000.0)
    } else {
        ("μs", 1.0)
    };

    let max_time_scaled = max_time as f64 / scale;
    let ticks = compute_nice_ticks(0.0, max_time_scaled);

    // Chart title with unit
    let title = Text::new(format!("{} ({})", benchmark.name, unit))
        .set("x", plot_x)
        .set("y", y_offset + 20.0)
        .set("font-family", "monospace")
        .set("font-size", 14)
        .set("font-weight", "bold");
    group = group.add(title);

    // Draw horizontal grid lines and y-axis labels
    for &tick in &ticks {
        let y = plot_y + plot_height - (tick / max_time_scaled) * plot_height;

        // Grid line
        let grid_line = Line::new()
            .set("x1", plot_x)
            .set("y1", y)
            .set("x2", plot_x + plot_width)
            .set("y2", y)
            .set("stroke", "#ddd")
            .set("stroke-width", 1);
        group = group.add(grid_line);

        // Tick label
        let label = Text::new(format!("{}", tick as i64))
            .set("x", plot_x - 8.0)
            .set("y", y + 4.0)
            .set("font-family", "monospace")
            .set("font-size", 11)
            .set("text-anchor", "end");
        group = group.add(label);
    }

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

    // Generate colors (oldest=lightest to newest=darkest)
    let colors = generate_shade_colors(base_hue, runs.len());

    // Create a group for all data lines with clip path applied
    // Store plot bounds as data attributes for zoom calculations
    let mut lines_group = Group::new()
        .set("id", format!("lines-{}", chart_id))
        .set("clip-path", format!("url(#clip-{})", chart_id))
        .set("data-plot-x", plot_x)
        .set("data-plot-y", plot_y)
        .set("data-plot-width", plot_width)
        .set("data-plot-height", plot_height)
        .set("data-min-t", 0.0)
        .set("data-max-t", 1.0)
        .set("data-min-time", 0.0)
        .set("data-max-time", max_time_scaled);

    // Draw data lines (oldest first so newest renders on top)
    for (i, run) in runs.iter().enumerate() {
        let color = &colors[i];
        let line_id = format!("line-{}-{}", chart_id, run.sha);

        if run.frames.is_empty() {
            continue;
        }

        // Store original data as JSON for zoom recalculation
        let data_points: Vec<(f64, f64)> = run
            .frames
            .iter()
            .map(|frame| (frame.t as f64, frame.time_us as f64 / scale))
            .collect();
        let data_json = serde_json::to_string(&data_points).unwrap_or_default();

        // Use t (0-1 progress) for x-axis, time_us for y-axis
        let points: Vec<(f64, f64)> = run
            .frames
            .iter()
            .map(|frame| {
                let x = plot_x + (frame.t as f64) * plot_width;
                let y =
                    plot_y + plot_height - (frame.time_us as f64 / max_time as f64) * plot_height;
                (x, y)
            })
            .collect();

        let points_str: String = points
            .iter()
            .map(|(x, y)| format!("{:.1},{:.1}", x, y))
            .collect::<Vec<_>>()
            .join(" ");

        let polyline = Polyline::new()
            .set("id", line_id)
            .set("class", "data-line")
            .set("data-points", data_json)
            .set("points", points_str)
            .set("fill", "none")
            .set("stroke", color.as_str())
            .set("stroke-width", 1.5);
        lines_group = lines_group.add(polyline);
    }
    group = group.add(lines_group);

    // Selection rectangle (hidden by default)
    let selection_rect = Rectangle::new()
        .set("id", format!("selection-{}", chart_id))
        .set("class", "selection-rect")
        .set("x", 0)
        .set("y", 0)
        .set("width", 0)
        .set("height", 0)
        .set("fill", "rgba(180, 180, 180, 0.3)")
        .set("stroke", "none")
        .set("visibility", "hidden");
    group = group.add(selection_rect);

    // Draw legend (newest first at top, matching visual prominence)
    let legend_x = plot_x + plot_width + 15.0;
    let legend_y = plot_y;

    // Reset zoom button (above legend, aligned with SHA text)
    let reset_btn = Text::new("[reset zoom]")
        .set("id", format!("reset-{}", chart_id))
        .set("class", "reset-zoom")
        .set("data-chart-id", chart_id.as_str())
        .set("x", legend_x + 16.0)
        .set("y", legend_y - 10.0)
        .set("font-family", "monospace")
        .set("font-size", 12)
        .set("fill", "#666")
        .set("visibility", "hidden");
    group = group.add(reset_btn);

    for (i, run) in runs.iter().rev().enumerate() {
        let color_idx = runs.len() - 1 - i; // Map back to color index
        let color = &colors[color_idx];
        let y = legend_y + (i as f64) * LEGEND_LINE_HEIGHT;
        let line_id = format!("line-{}-{}", chart_id, run.sha);
        let swatch_id = format!("swatch-{}-{}", chart_id, run.sha);

        // Color swatch (not clickable)
        let swatch = Rectangle::new()
            .set("id", swatch_id.clone())
            .set("x", legend_x)
            .set("y", y)
            .set("width", 12)
            .set("height", 12)
            .set("fill", color.as_str())
            .set("stroke", color.as_str())
            .set("stroke-width", 2);
        group = group.add(swatch);

        // SHA label (clickable)
        let label = Text::new(run.sha.as_str())
            .set("class", "legend-item")
            .set("data-line-id", line_id)
            .set("data-swatch-id", swatch_id)
            .set("data-color", color.as_str())
            .set("x", legend_x + 16.0)
            .set("y", y + 10.0)
            .set("font-family", "monospace")
            .set("font-size", 12);
        group = group.add(label);
    }

    // Add an invisible rectangle over the plot area to capture mouse events for zoom
    let plot_area = Rectangle::new()
        .set("class", "plot-area")
        .set("data-chart-id", chart_id.as_str())
        .set("x", plot_x)
        .set("y", plot_y)
        .set("width", plot_width)
        .set("height", plot_height)
        .set("fill", "transparent");
    group = group.add(plot_area);

    group
}

/// CSS for interactive elements.
const CHART_CSS: &str = r#"
.legend-item { cursor: pointer; user-select: none; }
.legend-item:hover { opacity: 0.8; }
.reset-zoom { cursor: pointer; user-select: none; }
.reset-zoom:hover { fill: #333 !important; }
.plot-area { cursor: crosshair; }
"#;

/// JavaScript for toggle and zoom functionality.
const CHART_JS: &str = r#"
// Toggle line visibility
document.querySelectorAll('.legend-item').forEach(item => {
    item.addEventListener('click', () => {
        const lineId = item.getAttribute('data-line-id');
        const swatchId = item.getAttribute('data-swatch-id');
        const color = item.getAttribute('data-color');
        const line = document.getElementById(lineId);
        const swatch = document.getElementById(swatchId);

        if (line.style.display === 'none') {
            line.style.display = '';
            swatch.setAttribute('fill', color);
        } else {
            line.style.display = 'none';
            swatch.setAttribute('fill', 'none');
        }
    });
});

// Zoom functionality
(function() {
    let isDragging = false;
    let startX, startY;
    let currentChartId = null;
    let selectionRect = null;

    function getMousePos(svg, evt) {
        const CTM = svg.getScreenCTM();
        return {
            x: (evt.clientX - CTM.e) / CTM.a,
            y: (evt.clientY - CTM.f) / CTM.d
        };
    }

    function updateLines(chartId, minT, maxT, minTime, maxTime) {
        const linesGroup = document.getElementById('lines-' + chartId);
        if (!linesGroup) return;

        const plotX = parseFloat(linesGroup.getAttribute('data-plot-x'));
        const plotY = parseFloat(linesGroup.getAttribute('data-plot-y'));
        const plotWidth = parseFloat(linesGroup.getAttribute('data-plot-width'));
        const plotHeight = parseFloat(linesGroup.getAttribute('data-plot-height'));

        // Update stored zoom bounds
        linesGroup.setAttribute('data-min-t', minT);
        linesGroup.setAttribute('data-max-t', maxT);
        linesGroup.setAttribute('data-min-time', minTime);
        linesGroup.setAttribute('data-max-time', maxTime);

        // Update each line
        linesGroup.querySelectorAll('.data-line').forEach(line => {
            const dataPoints = JSON.parse(line.getAttribute('data-points'));
            const tRange = maxT - minT;
            const timeRange = maxTime - minTime;

            const points = dataPoints.map(p => {
                const t = p[0];
                const time = p[1];
                const x = plotX + ((t - minT) / tRange) * plotWidth;
                const y = plotY + plotHeight - ((time - minTime) / timeRange) * plotHeight;
                return x.toFixed(1) + ',' + y.toFixed(1);
            }).join(' ');

            line.setAttribute('points', points);
        });

        // Show reset button
        const resetBtn = document.getElementById('reset-' + chartId);
        if (resetBtn) {
            resetBtn.setAttribute('visibility', 'visible');
        }
    }

    function resetZoom(chartId) {
        const linesGroup = document.getElementById('lines-' + chartId);
        if (!linesGroup) return;

        // Get original bounds from the data
        const lines = linesGroup.querySelectorAll('.data-line');
        let maxTime = 0;
        lines.forEach(line => {
            const dataPoints = JSON.parse(line.getAttribute('data-points'));
            dataPoints.forEach(p => {
                if (p[1] > maxTime) maxTime = p[1];
            });
        });

        updateLines(chartId, 0, 1, 0, maxTime);

        // Hide reset button
        const resetBtn = document.getElementById('reset-' + chartId);
        if (resetBtn) {
            resetBtn.setAttribute('visibility', 'hidden');
        }
    }

    // Reset button click handlers
    document.querySelectorAll('.reset-zoom').forEach(btn => {
        btn.addEventListener('click', () => {
            const chartId = btn.getAttribute('data-chart-id');
            resetZoom(chartId);
        });
    });

    // Plot area mouse handlers
    document.querySelectorAll('.plot-area').forEach(plotArea => {
        const chartId = plotArea.getAttribute('data-chart-id');
        const svg = plotArea.ownerSVGElement;

        plotArea.addEventListener('mousedown', (evt) => {
            isDragging = true;
            currentChartId = chartId;
            const pos = getMousePos(svg, evt);
            startX = pos.x;
            startY = pos.y;

            selectionRect = document.getElementById('selection-' + chartId);
            if (selectionRect) {
                selectionRect.setAttribute('x', startX);
                selectionRect.setAttribute('y', startY);
                selectionRect.setAttribute('width', 0);
                selectionRect.setAttribute('height', 0);
                selectionRect.setAttribute('visibility', 'visible');
            }

            evt.preventDefault();
        });
    });

    document.addEventListener('mousemove', (evt) => {
        if (!isDragging || !selectionRect) return;

        const svg = selectionRect.ownerSVGElement;
        const pos = getMousePos(svg, evt);

        const x = Math.min(startX, pos.x);
        const y = Math.min(startY, pos.y);
        const width = Math.abs(pos.x - startX);
        const height = Math.abs(pos.y - startY);

        selectionRect.setAttribute('x', x);
        selectionRect.setAttribute('y', y);
        selectionRect.setAttribute('width', width);
        selectionRect.setAttribute('height', height);
    });

    document.addEventListener('mouseup', (evt) => {
        if (!isDragging || !currentChartId) return;

        isDragging = false;

        if (selectionRect) {
            selectionRect.setAttribute('visibility', 'hidden');

            const x = parseFloat(selectionRect.getAttribute('x'));
            const y = parseFloat(selectionRect.getAttribute('y'));
            const width = parseFloat(selectionRect.getAttribute('width'));
            const height = parseFloat(selectionRect.getAttribute('height'));

            // Only zoom if selection is large enough
            if (width > 5 && height > 5) {
                const linesGroup = document.getElementById('lines-' + currentChartId);
                if (linesGroup) {
                    const plotX = parseFloat(linesGroup.getAttribute('data-plot-x'));
                    const plotY = parseFloat(linesGroup.getAttribute('data-plot-y'));
                    const plotWidth = parseFloat(linesGroup.getAttribute('data-plot-width'));
                    const plotHeight = parseFloat(linesGroup.getAttribute('data-plot-height'));
                    const currentMinT = parseFloat(linesGroup.getAttribute('data-min-t'));
                    const currentMaxT = parseFloat(linesGroup.getAttribute('data-max-t'));
                    const currentMinTime = parseFloat(linesGroup.getAttribute('data-min-time'));
                    const currentMaxTime = parseFloat(linesGroup.getAttribute('data-max-time'));

                    // Convert pixel coords to data coords
                    const tRange = currentMaxT - currentMinT;
                    const timeRange = currentMaxTime - currentMinTime;

                    const newMinT = currentMinT + ((x - plotX) / plotWidth) * tRange;
                    const newMaxT = currentMinT + ((x + width - plotX) / plotWidth) * tRange;
                    // Y is inverted (top = high values)
                    const newMaxTime = currentMaxTime - ((y - plotY) / plotHeight) * timeRange;
                    const newMinTime = currentMaxTime - ((y + height - plotY) / plotHeight) * timeRange;

                    updateLines(currentChartId, newMinT, newMaxT, newMinTime, newMaxTime);
                }
            }
        }

        currentChartId = null;
        selectionRect = null;
    });
})();
"#;

/// Generate a complete SVG with charts for all benchmarks.
///
/// Takes the output of `load_all_benchmarks`: a list of BenchmarkData.
pub fn generate_svg(benchmarks: &[BenchmarkData]) -> String {
    // Calculate total dimensions
    let total_height = benchmarks.len() as f64 * (CHART_HEIGHT + CHART_SPACING);
    let total_width = CHART_WIDTH + 120.0; // Extra space for legend

    // Create document
    let mut document =
        Document::new().set("viewBox", (0, 0, total_width as i32, total_height as i32));

    // Add CSS
    let style = Style::new(CHART_CSS);
    document = document.add(style);

    // Background
    let background = Rectangle::new()
        .set("width", "100%")
        .set("height", "100%")
        .set("fill", "#fafafa");
    document = document.add(background);

    // Generate each chart
    for (i, benchmark) in benchmarks.iter().enumerate() {
        let y_offset = i as f64 * (CHART_HEIGHT + CHART_SPACING);
        let base_hue = BASE_HUES[i % BASE_HUES.len()];
        let chart_group = generate_benchmark_chart(benchmark, base_hue, y_offset);
        document = document.add(chart_group);
    }

    // Add JavaScript at the end
    let script = Script::new(CHART_JS);
    document = document.add(script);

    document.to_string()
}
