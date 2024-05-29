use std::borrow::Borrow;

use plotters::backend::SVGBackend;
use plotters::chart::ChartBuilder;
use plotters::coord::ranged1d::IntoSegmentedCoord;
use plotters::drawing::IntoDrawingArea;
use plotters::series::Histogram;
use plotters::style::full_palette::{RED, WHITE};
use plotters::style::text_anchor::{HPos, Pos, VPos};
use plotters::style::{Color, FontTransform, IntoFont, TextStyle};

use super::replay_statistics::ReplayStatistics;

pub fn export_histogram(
    filename: &str,
    title: &str,
    libfunc_stats: ReplayStatistics,
) -> anyhow::Result<()> {
    let list_of_libfuncs: Vec<&str> = libfunc_stats
        .concrete_libfunc
        .keys()
        .map(|s| s.as_str())
        .collect::<Vec<&str>>();
    let max_label_size = 550; //pixels
    let number_of_libfuncs = list_of_libfuncs.len();
    tracing::info!("Number of libfuncs {number_of_libfuncs}");
    let max_y_axis = ((libfunc_stats
        .concrete_libfunc
        .values()
        .max()
        .unwrap()
        .clone() as f32
        / 10.0)
        .floor()
        + 1.0) as usize
        * 10;
    tracing::info!("Max y axis {max_y_axis}");
    let width: u32 = (number_of_libfuncs as u32) * 40 + 250; //pixels
    let height: u32 = (max_y_axis as u32) * 2 + max_label_size;

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
                    libfunc_stats
                        .concrete_libfunc
                        .get::<str>(x.borrow())
                        .unwrap()
                        .clone(),
                )
            })),
    )?;

    // To avoid the IO failure being ignored silently, we manually call the
    // present function
    root.present().expect("Unable to write result to file");
    Ok(())
}
