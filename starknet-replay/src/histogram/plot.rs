//! This module uses the library `plotter` to render and export the histogram.
//!
//! To use another plotting library, it's sufficient to rewrite the function
//! `render`.

use plotters::backend::SVGBackend;
use plotters::chart::ChartBuilder;
use plotters::coord::ranged1d::{IntoSegmentedCoord, SegmentValue};
use plotters::drawing::IntoDrawingArea;
use plotters::series::Histogram;
use plotters::style::full_palette::{RED, WHITE};
use plotters::style::text_anchor::{HPos, Pos, VPos};
use plotters::style::{Color, FontTransform, IntoFont, TextStyle};

use crate::histogram::{Config, HistogramError};
use crate::ReplayStatistics;

/// Create and export the histogram as SVG file.
///
/// # Arguments
///
/// - `title`: The title of the histogram.
/// - `config`: The configuration object of the histogram.
/// - `libfunc_stats`: The input data to be plotted.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - There is an error computing the histogram `Config` object.
/// - There is an error rendering the histogram.
pub fn render(
    title: &str,
    config: &Config,
    libfunc_stats: &ReplayStatistics,
) -> Result<String, HistogramError> {
    let mut buffer: String = String::default();
    render_with_plotter(title, config, libfunc_stats, &mut buffer)?;
    Ok(buffer.clone())
}

/// Internal function to call `plotter` rendering backend.
///
/// # Arguments
///
/// - `title`: The title of the histogram.
/// - `config`: The configuration object of the histogram.
/// - `libfunc_stats`: The input data to be plotted.
/// - `buffer`: The buffer that contains the SVG image.
fn render_with_plotter(
    title: &str,
    config: &Config,
    libfunc_stats: &ReplayStatistics,
    buffer: &mut String,
) -> Result<(), HistogramError> {
    let list_of_libfuncs = libfunc_stats.get_libfuncs();
    let root = SVGBackend::with_string(buffer, (config.width, config.height)).into_drawing_area();

    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(config.x_label_area)
        .y_label_area_size(150)
        .margin(30)
        .caption(title, ("sans-serif", 50.0))
        .build_cartesian_2d(
            list_of_libfuncs.as_slice().into_segmented(),
            0..config.max_y_axis,
        )?;

    // The use of `x_label_formatter` ensures labels aren't printed with quotes
    // around them.
    chart
        .configure_mesh()
        .x_labels(libfunc_stats.get_number_of_libfuncs())
        .x_label_formatter(&|pos| match pos {
            SegmentValue::Exact(t) | SegmentValue::CenterOf(t) => (**t).to_string(),
            SegmentValue::Last => String::new(),
        })
        .y_labels(config.max_y_axis / 100)
        .max_light_lines(1)
        .disable_x_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .y_desc("Frequency")
        .x_desc("Libfunc name")
        .x_label_style(
            // When rotating 90 deg, `HPos` controls the vertical position.
            // `VPos` controls the horizontal position.
            TextStyle::from(("sans-serif", 20).into_font())
                .transform(FontTransform::Rotate90)
                .pos(Pos::new(HPos::Left, VPos::Center)),
        )
        .axis_desc_style(("sans-serif", 35))
        .draw()?;

    chart.draw_series(
        Histogram::vertical(&chart)
            .style(RED.mix(0.5).filled())
            .data(list_of_libfuncs.iter().map(|libfunc_name| {
                let frequency = libfunc_stats.get_libfunc_frequency(libfunc_name);
                (libfunc_name, frequency)
            })),
    )?;

    Ok(())
}
