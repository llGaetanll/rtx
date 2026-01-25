// Toggle line visibility
document.querySelectorAll(".legend-item").forEach((item) => {
  item.addEventListener("click", () => {
    const lineId = item.getAttribute("data-line-id");
    const swatchId = item.getAttribute("data-swatch-id");
    const color = item.getAttribute("data-color");
    const line = document.getElementById(lineId);
    const swatch = document.getElementById(swatchId);

    if (line.style.display === "none") {
      line.style.display = "";
      swatch.setAttribute("fill", color);
    } else {
      line.style.display = "none";
      swatch.setAttribute("fill", "none");
    }
  });
});

// Zoom and tick functionality
(function () {
  const SVG_NS = "http://www.w3.org/2000/svg";
  const TARGET_TICKS = 5;

  let isDragging = false;
  let startX, startY;
  let currentChartId = null;
  let selectionRect = null;
  let plotBounds = null; // {x, y, width, height} for clamping selection

  function getMousePos(svg, evt) {
    const CTM = svg.getScreenCTM();
    return {
      x: (evt.clientX - CTM.e) / CTM.a,
      y: (evt.clientY - CTM.f) / CTM.d,
    };
  }

  function clamp(value, min, max) {
    return Math.max(min, Math.min(max, value));
  }

  // Compute nice tick positions for a given range
  function computeNiceTicks(min, max) {
    const range = max - min;
    if (range === 0) return [min];

    const rawStep = range / TARGET_TICKS;
    const magnitude = Math.pow(10, Math.floor(Math.log10(rawStep)));
    const normalizedStep = rawStep / magnitude;

    let niceStep;
    if (normalizedStep <= 1) niceStep = 1;
    else if (normalizedStep <= 2) niceStep = 2;
    else if (normalizedStep <= 5) niceStep = 5;
    else niceStep = 10;

    const step = niceStep * magnitude;
    const niceStart = Math.floor(min / step) * step;
    const niceEnd = Math.floor(max / step) * step;

    const ticks = [];
    for (let t = niceStart; t <= niceEnd + 1e-10; t += step) {
      // Only include ticks within the visible range
      if (t >= min - 1e-10) {
        ticks.push(t);
      }
    }
    return ticks;
  }

  // Update tick marks and labels for a chart
  function updateTicks(chartId, minFrame, maxFrame, minTime, maxTime) {
    const ticksGroup = document.getElementById("ticks-" + chartId);
    if (!ticksGroup) return;

    const plotX = parseFloat(ticksGroup.getAttribute("data-plot-x"));
    const plotY = parseFloat(ticksGroup.getAttribute("data-plot-y"));
    const plotWidth = parseFloat(ticksGroup.getAttribute("data-plot-width"));
    const plotHeight = parseFloat(ticksGroup.getAttribute("data-plot-height"));

    // Clear existing ticks
    while (ticksGroup.firstChild) {
      ticksGroup.removeChild(ticksGroup.firstChild);
    }

    // Y-axis ticks (time)
    const timeTicks = computeNiceTicks(minTime, maxTime);
    const timeRange = maxTime - minTime;

    timeTicks.forEach((tick) => {
      const y =
        plotY + plotHeight - ((tick - minTime) / timeRange) * plotHeight;

      // Grid line
      const gridLine = document.createElementNS(SVG_NS, "line");
      gridLine.setAttribute("x1", plotX);
      gridLine.setAttribute("y1", y);
      gridLine.setAttribute("x2", plotX + plotWidth);
      gridLine.setAttribute("y2", y);
      gridLine.setAttribute("stroke", "#ddd");
      gridLine.setAttribute("stroke-width", "1");
      ticksGroup.appendChild(gridLine);

      // Tick label - format nicely
      const label = document.createElementNS(SVG_NS, "text");
      label.setAttribute("x", plotX - 8);
      label.setAttribute("y", y + 4);
      label.setAttribute("font-family", "monospace");
      label.setAttribute("font-size", "11");
      label.setAttribute("text-anchor", "end");
      // Format: use integers if possible, otherwise 1 decimal
      const labelText =
        tick === Math.floor(tick) ? tick.toString() : tick.toFixed(1);
      label.textContent = labelText;
      ticksGroup.appendChild(label);
    });

    // X-axis ticks (frame numbers)
    const frameTicks = computeNiceTicks(minFrame, maxFrame);
    const frameRange = maxFrame - minFrame;

    frameTicks.forEach((tick) => {
      const x = plotX + ((tick - minFrame) / frameRange) * plotWidth;

      // Vertical grid line
      const gridLine = document.createElementNS(SVG_NS, "line");
      gridLine.setAttribute("x1", x);
      gridLine.setAttribute("y1", plotY);
      gridLine.setAttribute("x2", x);
      gridLine.setAttribute("y2", plotY + plotHeight);
      gridLine.setAttribute("stroke", "#ddd");
      gridLine.setAttribute("stroke-width", "1");
      ticksGroup.appendChild(gridLine);

      // Tick label below x-axis
      const label = document.createElementNS(SVG_NS, "text");
      label.setAttribute("x", x);
      label.setAttribute("y", plotY + plotHeight + 15);
      label.setAttribute("font-family", "monospace");
      label.setAttribute("font-size", "11");
      label.setAttribute("text-anchor", "middle");
      // Frame numbers are always integers
      label.textContent = Math.round(tick).toString();
      ticksGroup.appendChild(label);
    });
  }

  function updateChart(
    chartId,
    minFrame,
    maxFrame,
    minTime,
    maxTime,
    isZoomed,
  ) {
    const linesGroup = document.getElementById("lines-" + chartId);
    if (!linesGroup) return;

    const plotX = parseFloat(linesGroup.getAttribute("data-plot-x"));
    const plotY = parseFloat(linesGroup.getAttribute("data-plot-y"));
    const plotWidth = parseFloat(linesGroup.getAttribute("data-plot-width"));
    const plotHeight = parseFloat(linesGroup.getAttribute("data-plot-height"));

    // Update stored zoom bounds
    linesGroup.setAttribute("data-min-frame", minFrame);
    linesGroup.setAttribute("data-max-frame", maxFrame);
    linesGroup.setAttribute("data-min-time", minTime);
    linesGroup.setAttribute("data-max-time", maxTime);

    // Update each line
    linesGroup.querySelectorAll(".data-line").forEach((line) => {
      const dataPoints = JSON.parse(line.getAttribute("data-points"));
      const frameRange = maxFrame - minFrame;
      const timeRange = maxTime - minTime;

      const points = dataPoints
        .map((p) => {
          const frame = p[0];
          const time = p[1];
          const x = plotX + ((frame - minFrame) / frameRange) * plotWidth;
          const y =
            plotY + plotHeight - ((time - minTime) / timeRange) * plotHeight;
          return x.toFixed(1) + "," + y.toFixed(1);
        })
        .join(" ");

      line.setAttribute("points", points);
    });

    // Update ticks
    updateTicks(chartId, minFrame, maxFrame, minTime, maxTime);

    // Show/hide reset button
    const resetBtn = document.getElementById("reset-" + chartId);
    if (resetBtn) {
      resetBtn.setAttribute("visibility", isZoomed ? "visible" : "hidden");
    }
  }

  function resetZoom(chartId) {
    const linesGroup = document.getElementById("lines-" + chartId);
    if (!linesGroup) return;

    // Get original bounds from the data
    const lines = linesGroup.querySelectorAll(".data-line");
    let maxFrame = 0;
    let maxTime = 0;
    lines.forEach((line) => {
      const dataPoints = JSON.parse(line.getAttribute("data-points"));
      dataPoints.forEach((p) => {
        if (p[0] > maxFrame) maxFrame = p[0];
        if (p[1] > maxTime) maxTime = p[1];
      });
    });

    updateChart(chartId, 0, maxFrame, 0, maxTime, false);
  }

  // Initialize ticks on page load
  function initializeAllCharts() {
    document.querySelectorAll('[id^="lines-"]').forEach((linesGroup) => {
      const chartId = linesGroup.id.replace("lines-", "");
      const maxFrame = parseFloat(linesGroup.getAttribute("data-max-frame"));
      const maxTime = parseFloat(linesGroup.getAttribute("data-max-time"));
      updateTicks(chartId, 0, maxFrame, 0, maxTime);
    });
  }

  // Reset button click handlers
  document.querySelectorAll(".reset-zoom").forEach((btn) => {
    btn.addEventListener("click", () => {
      const chartId = btn.getAttribute("data-chart-id");
      resetZoom(chartId);
    });
  });

  // Plot area mouse handlers
  document.querySelectorAll(".plot-area").forEach((plotArea) => {
    const chartId = plotArea.getAttribute("data-chart-id");
    const svg = plotArea.ownerSVGElement;

    plotArea.addEventListener("mousedown", (evt) => {
      isDragging = true;
      currentChartId = chartId;
      const pos = getMousePos(svg, evt);

      // Store plot bounds for clamping
      const linesGroup = document.getElementById("lines-" + chartId);
      if (linesGroup) {
        plotBounds = {
          x: parseFloat(linesGroup.getAttribute("data-plot-x")),
          y: parseFloat(linesGroup.getAttribute("data-plot-y")),
          width: parseFloat(linesGroup.getAttribute("data-plot-width")),
          height: parseFloat(linesGroup.getAttribute("data-plot-height")),
        };
      }

      // Clamp start position to plot bounds
      startX = clamp(pos.x, plotBounds.x, plotBounds.x + plotBounds.width);
      startY = clamp(pos.y, plotBounds.y, plotBounds.y + plotBounds.height);

      selectionRect = document.getElementById("selection-" + chartId);
      if (selectionRect) {
        selectionRect.setAttribute("x", startX);
        selectionRect.setAttribute("y", startY);
        selectionRect.setAttribute("width", 0);
        selectionRect.setAttribute("height", 0);
        selectionRect.setAttribute("visibility", "visible");
      }

      evt.preventDefault();
    });
  });

  document.addEventListener("mousemove", (evt) => {
    if (!isDragging || !selectionRect || !plotBounds) return;

    const svg = selectionRect.ownerSVGElement;
    const pos = getMousePos(svg, evt);

    // Clamp current position to plot bounds
    const clampedX = clamp(
      pos.x,
      plotBounds.x,
      plotBounds.x + plotBounds.width,
    );
    const clampedY = clamp(
      pos.y,
      plotBounds.y,
      plotBounds.y + plotBounds.height,
    );

    const x = Math.min(startX, clampedX);
    const y = Math.min(startY, clampedY);
    const width = Math.abs(clampedX - startX);
    const height = Math.abs(clampedY - startY);

    selectionRect.setAttribute("x", x);
    selectionRect.setAttribute("y", y);
    selectionRect.setAttribute("width", width);
    selectionRect.setAttribute("height", height);
  });

  document.addEventListener("mouseup", (evt) => {
    if (!isDragging || !currentChartId) return;

    isDragging = false;

    if (selectionRect) {
      selectionRect.setAttribute("visibility", "hidden");

      const x = parseFloat(selectionRect.getAttribute("x"));
      const y = parseFloat(selectionRect.getAttribute("y"));
      const width = parseFloat(selectionRect.getAttribute("width"));
      const height = parseFloat(selectionRect.getAttribute("height"));

      // Only zoom if selection is large enough
      if (width > 5 && height > 5) {
        const linesGroup = document.getElementById("lines-" + currentChartId);
        if (linesGroup) {
          const plotX = parseFloat(linesGroup.getAttribute("data-plot-x"));
          const plotY = parseFloat(linesGroup.getAttribute("data-plot-y"));
          const plotWidth = parseFloat(
            linesGroup.getAttribute("data-plot-width"),
          );
          const plotHeight = parseFloat(
            linesGroup.getAttribute("data-plot-height"),
          );
          const currentMinFrame = parseFloat(
            linesGroup.getAttribute("data-min-frame"),
          );
          const currentMaxFrame = parseFloat(
            linesGroup.getAttribute("data-max-frame"),
          );
          const currentMinTime = parseFloat(
            linesGroup.getAttribute("data-min-time"),
          );
          const currentMaxTime = parseFloat(
            linesGroup.getAttribute("data-max-time"),
          );

          // Convert pixel coords to data coords
          const frameRange = currentMaxFrame - currentMinFrame;
          const timeRange = currentMaxTime - currentMinTime;

          const newMinFrame =
            currentMinFrame + ((x - plotX) / plotWidth) * frameRange;
          const newMaxFrame =
            currentMinFrame + ((x + width - plotX) / plotWidth) * frameRange;
          // Y is inverted (top = high values)
          const newMaxTime =
            currentMaxTime - ((y - plotY) / plotHeight) * timeRange;
          const newMinTime =
            currentMaxTime - ((y + height - plotY) / plotHeight) * timeRange;

          updateChart(
            currentChartId,
            newMinFrame,
            newMaxFrame,
            newMinTime,
            newMaxTime,
            true,
          );
        }
      }
    }

    currentChartId = null;
    selectionRect = null;
    plotBounds = null;
  });

  // Initialize on load
  initializeAllCharts();
})();
