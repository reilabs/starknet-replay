//! This module is responsible for rendering and saving the SVG image of the
//! libfunc histogram.
//!
//! It uses the `plotter` library to generate the histogram
//! and rendering.
//!
//! SVG has been chosen because, as a vector graphics format, it makes reading
//! text easy and zooming doesn't degrade the quality.

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

/// This struct contains the variable configuration parameters for rendering the
/// histogram image.
///
/// Configuration parameters that don't depend on `ReplayStatistics` are
/// hardcoded.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
    /// The width of the SVG image of the histogram in pixels.
    pub width: Pixel,

    /// The height of the SVG image of the histogram in pixels.
    pub height: Pixel,

    /// The max number shown on the y axis of the histogram.
    pub max_y_axis: usize,

    /// Number of pixels used below the x-axis for the labels.
    pub x_label_area: Pixel,
}
impl Config {
    /// Construct a new `Config` object.
    ///
    /// # Arguments
    ///
    /// - `libfunc_stats`: the data to be plotted on the histogram.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if:
    ///
    /// - There is a math overflow when computing the `Config` parameters
    /// - There is a truncation when casting from `usize` to `u32`.
    pub fn new(libfunc_stats: &ReplayStatistics) -> Result<Self, HistogramError> {
        let max_frequency = libfunc_stats
            .get_highest_frequency()
            .ok_or(HistogramError::Empty)?;
        let number_of_buckets = libfunc_stats.get_number_of_libfuncs();
        let x_label_area = Self::calc_x_label_area(libfunc_stats)?;
        let width = Self::calc_width(number_of_buckets)?;
        let max_y_axis = Self::calc_max_y_axis(max_frequency)?;
        let height = Self::calc_height(max_y_axis, x_label_area)?;

        tracing::info!("Number of buckets {number_of_buckets}");
        tracing::info!("Max y axis {max_y_axis}");

        Ok(Config {
            width,
            height,
            max_y_axis,
            x_label_area,
        })
    }

    /// Calculate the space required required to render the x axis labels.
    ///
    /// The space is returned in pixels.
    ///
    /// # Arguments
    ///
    /// - `libfunc_stats`: data to be plotted on the histogram.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if:
    ///
    /// - There is a math overflow when computing the number of pixels.
    /// - There is a truncation when casting from `usize` to `u32`.
    fn calc_x_label_area(libfunc_stats: &ReplayStatistics) -> Result<Pixel, HistogramError> {
        let chars_longest_name: usize = libfunc_stats
            .get_libfuncs()
            .iter()
            .max_by_key(|p| p.len())
            .unwrap_or(&"")
            .len();
        let pixels_per_char: usize = 10;
        let x_label_area_size: usize =
            chars_longest_name
                .checked_mul(pixels_per_char)
                .ok_or(HistogramError::MathOverflow(
                    "calc_x_label_area".to_string(),
                ))?;
        Ok(u32::try_from(x_label_area_size)?)
    }

    /// Calculate the maximum number shown on the y-axis of the histogram.
    ///
    /// The principle is to automatically resize the axis depending on the
    /// number of times the most frequently called `libfunc` is called. This
    /// number is rounded to the next hundreds.
    ///
    /// # Arguments
    ///
    /// - `max_frequency`: The highest frequency to be represented in the
    ///   histogram.
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

    /// Calculate the height in pixels of the SVG image containing the
    /// histogram.
    ///
    /// The idea is to provide 2 pixels of height for buckets with frequency of
    /// 1 plus some margin at the bottom for the labels of the x-axis.
    ///
    /// # Arguments
    ///
    /// - `max_y_axis`: The max extension of the y-axis.
    /// - `x_axis_label_space`: The amount of margin for x-axis labels.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if there is an overflow in the calculation of the
    /// height.
    fn calc_height(max_y_axis: usize, x_axis_label_space: Pixel) -> Result<Pixel, HistogramError> {
        let max_y_axis: Pixel = u32::try_from(max_y_axis)?;
        let pixels_for_each_call: Pixel = 2;
        max_y_axis
            .checked_mul(pixels_for_each_call)
            .and_then(|n| n.checked_add(x_axis_label_space))
            .ok_or(HistogramError::MathOverflow("calc_height".to_string()))
    }
}

/// Create and export the histogram as SVG file.
///
/// # Arguments
///
/// - `filename`: The filename of the exported SVG file.
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
/// - There is an IO error saving the SVG file of the histogram.
fn render(
    filename: &PathBuf,
    title: &str,
    config: &Config,
    libfunc_stats: &ReplayStatistics,
) -> Result<(), HistogramError> {
    let list_of_libfuncs = libfunc_stats.get_libfuncs();
    let root = SVGBackend::new(filename, (config.width, config.height)).into_drawing_area();

    root.fill(&WHITE)?;

    // Putting spaces in the caption creates panic
    // https://github.com/plotters-rs/plotters/issues/573#issuecomment-2096057443
    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(config.x_label_area)
        .y_label_area_size(40)
        .margin(5)
        .caption(title, ("sans-serif", 50.0))
        .build_cartesian_2d(
            list_of_libfuncs.as_slice().into_segmented(),
            0..config.max_y_axis,
        )?;

    chart
        .configure_mesh()
        .x_labels(libfunc_stats.get_number_of_libfuncs())
        .y_labels(config.max_y_axis / 100)
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
            .data(list_of_libfuncs.iter().map(|libfunc_name| {
                let frequency = libfunc_stats.get_libfunc_frequency(libfunc_name);
                (libfunc_name, frequency)
            })),
    )?;

    // To avoid the IO failure being ignored silently, we manually call the
    // present function
    root.present()?;
    Ok(())
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
    let config = Config::new(libfunc_stats)?;

    render(filename, title, &config, libfunc_stats)
}

#[cfg(test)]
mod tests {
    use rand::distributions::{Alphanumeric, DistString};
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    use super::*;

    fn generate_dummy_replay_statistics(
        string_len: usize,
        number_libfuncs: usize,
        max_frequency: usize,
    ) -> ReplayStatistics {
        // Deterministic seed in order to have the same sequence of pseud-random
        // numbers.
        let mut replay_statistics = ReplayStatistics::default();
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        (0..number_libfuncs).for_each(|_| {
            let libfunc_name = Alphanumeric.sample_string(&mut rng, string_len);
            let libfunc_frequency = rng.gen_range(0..max_frequency);
            replay_statistics
                .concrete_libfunc
                .insert(libfunc_name, libfunc_frequency);
        });

        replay_statistics
    }

    #[test]
    fn test_calc_x_label_area() {
        let string_len = 20;
        let number_libfuncs = 130;
        let max_frequency = 1600;
        let replay_statistics =
            generate_dummy_replay_statistics(string_len, number_libfuncs, max_frequency);
        let x_label_area = Config::calc_x_label_area(&replay_statistics).unwrap();
        let expected_x_label_area = 200;
        assert_eq!(x_label_area, expected_x_label_area);
    }

    #[test]
    fn test_calc_max_y_axis() {
        let max_frequency = 131;
        let max_y_axis = Config::calc_max_y_axis(max_frequency).unwrap();
        let expected_max_y_axis = 200;
        assert_eq!(max_y_axis, expected_max_y_axis);
    }

    #[test]
    fn test_calc_width() {
        let number_libfuncs = 130;
        let width = Config::calc_width(number_libfuncs).unwrap();
        let expected_width = 5450;
        assert_eq!(width, expected_width);
    }

    #[test]
    fn test_calc_height() {
        let max_y_axis = 130;
        let x_axis_label_space = 250;
        let height = Config::calc_height(max_y_axis, x_axis_label_space).unwrap();
        let expected_height = 510;
        assert_eq!(height, expected_height);
    }
}
