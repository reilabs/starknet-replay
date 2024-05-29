//! This module is responsible for rendering and saving the SVG image of the
//! libfunc histogram.
//!
//! It uses the `plotter` library to generate the histogram
//! and rendering.
//!
//! SVG has been chosen because, as a vector graphics format, it makes reading
//! text easy and zooming doesn't degrade the quality.

use std::borrow::Borrow;
use std::ops::{Add, Div};
use std::path::PathBuf;

use plotters::backend::SVGBackend;
use plotters::chart::ChartBuilder;
use plotters::coord::ranged1d::IntoSegmentedCoord;
use plotters::drawing::IntoDrawingArea;
use plotters::series::Histogram;
use plotters::style::full_palette::{RED, WHITE};
use plotters::style::text_anchor::{HPos, Pos, VPos};
use plotters::style::{Color, FontTransform, IntoFont, TextStyle};

use super::replay_statistics::ReplayStatistics;
use crate::error::HistogramError;

/// This alias improves readability of the histogram parameters.
type Pixel = u32;

// struct HistogramConfig {
//     width: Pixel,
//     height: Pixel,
//     max_y_axis: usize,
// }

/// Calculate the maximum number shown on the y-axis of the histogram.
///
/// The principle is to automatically resize the axis depending on the number of
/// times the most frequently called `libfunc` is called. This number is rounded
/// to the next hundreds.
///
/// # Arguments
///
/// - `max_frequency`: The highest frequency to be represented in the histogram.
///
/// # Errors
///
/// Returns [`Err`] if there is an overflow in the calculation of the y-axis
/// extension.
fn calc_max_y_axis(max_frequency: usize) -> Result<usize, HistogramError> {
    // `div` and `add` don't need to be checked because they will always return a
    // number less than `max_frequency`, therefore fitting in the size of `usize`.
    let max_y_axis = max_frequency.div(100).add(1).checked_mul(100);
    max_y_axis.ok_or(HistogramError::MathOverflow("calc_max_y_axis".to_string()))
}

/// Calculate the width in pixels of the SVG image containing the histogram.
///
/// The idea is to resize the width depending on the number of buckets to be
/// plotted plus some margin.
///
/// # Arguments
///
/// - `number_of_buckets`: The number of buckets in the histogram.
///
/// # Errors
///
/// Returns [`Err`] if there is an overflow in the calculation of the width.
fn calc_width(number_of_buckets: usize) -> Result<Pixel, HistogramError> {
    let number_of_buckets: Pixel = u32::try_from(number_of_buckets)?;
    let pixels_per_bucket: Pixel = 40;
    let extra_margins: Pixel = 250;
    number_of_buckets
        .checked_mul(pixels_per_bucket)
        .and_then(|n| n.checked_add(extra_margins))
        .ok_or(HistogramError::MathOverflow("calc_width".to_string()))
}

/// Calculate the height in pixels of the SVG image containing the histogram.
///
/// The idea is to provide 2 pixels of height for buckets with frequency of 1
/// plus some margin at the bottom for the labels of the x-axis.
///
/// # Arguments
///
/// - `max_y_axis`: The max extension of the y-axis.
/// - `x_axis_label_space`: The amount of margin for x-axis labels.
///
/// # Errors
///
/// Returns [`Err`] if there is an overflow in the calculation of the height.
fn calc_height(max_y_axis: usize, x_axis_label_space: Pixel) -> Result<Pixel, HistogramError> {
    let max_y_axis: Pixel = u32::try_from(max_y_axis)?;
    let pixels_for_each_call: Pixel = 2;
    max_y_axis
        .checked_mul(pixels_for_each_call)
        .and_then(|n| n.checked_add(x_axis_label_space))
        .ok_or(HistogramError::MathOverflow("calc_height".to_string()))
}

/// This function generates and saves the libfunc frequency histogram.
///
/// # Arguments
///
/// - `filename`: The filename to output the SVG.
/// - `title`: The title of the histogram.
/// - `libfunc_stats`: The object containing libfunc statistics.
///
/// # Errors
///
/// Returns [`Err`] if `filename` can't be written to or if there is any error
/// rendering the data.
pub fn export(
    filename: &PathBuf,
    title: &str,
    libfunc_stats: &ReplayStatistics,
) -> Result<(), HistogramError> {
    let list_of_libfuncs: Vec<&str> = libfunc_stats
        .concrete_libfunc
        .keys()
        .map(std::string::String::as_str)
        .collect::<Vec<&str>>();
    let max_label_size: Pixel = 550;
    let number_of_libfuncs = list_of_libfuncs.len();
    tracing::info!("Number of libfuncs {number_of_libfuncs}");
    let max_frequency: usize = *libfunc_stats
        .concrete_libfunc
        .values()
        .max()
        .ok_or(HistogramError::Empty)?;
    let max_y_axis: usize = calc_max_y_axis(max_frequency)?;
    tracing::info!("Max y axis {max_y_axis}");
    let width: Pixel = calc_width(number_of_libfuncs)?; //pixels
    let height: Pixel = calc_height(max_y_axis, max_label_size)?;

    let root = SVGBackend::new(filename, (width, height)).into_drawing_area();

    root.fill(&WHITE)?;

    // Putting spaces in the caption creates panic
    // https://github.com/plotters-rs/plotters/issues/573#issuecomment-2096057443
    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(max_label_size)
        .y_label_area_size(40)
        .margin(5)
        .caption(title, ("sans-serif", 50.0))
        .build_cartesian_2d(
            list_of_libfuncs.as_slice().into_segmented(),
            0usize..max_y_axis,
        )?;

    chart
        .configure_mesh()
        .x_labels(number_of_libfuncs)
        .y_labels(max_y_axis / 100)
        .max_light_lines(1)
        .disable_x_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .y_desc("Frequency")
        .x_desc("Libfunc")
        .x_label_style(
            TextStyle::from(("sans-serif", 20).into_font())
                .transform(FontTransform::Rotate90)
                .pos(Pos::new(HPos::Center, VPos::Top)),
        )
        .axis_desc_style(("sans-serif", 15))
        .draw()?;

    chart.draw_series(
        Histogram::vertical(&chart)
            .style(RED.mix(0.5).filled())
            .data(list_of_libfuncs.iter().map(|x| {
                (
                    x,
                    *libfunc_stats
                        .concrete_libfunc
                        .get::<str>(x.borrow())
                        .unwrap(),
                )
            })),
    )?;

    // To avoid the IO failure being ignored silently, we manually call the
    // present function
    root.present().expect("Unable to write result to file");
    Ok(())
}
