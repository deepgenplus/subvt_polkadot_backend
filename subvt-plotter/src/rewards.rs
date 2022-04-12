use crate::plotlib::{AxisPosition, Chart, ScaleBand, ScaleLinear, VerticalBarView};
use crate::{PlotterError, CONFIG};
use chrono::Datelike;
use itertools::Itertools;
use std::collections::HashMap;
use std::path::PathBuf;
use subvt_types::substrate::{Balance, Era};
use subvt_utility::numeric::format_decimal;

fn get_monthly_rewards(rewards: &[(Era, Balance)]) -> anyhow::Result<HashMap<u32, Balance>> {
    if rewards.is_empty() {
        return Err(PlotterError::EmptyData.into());
    }
    let mut monthly_rewards: HashMap<u32, Balance> = HashMap::new();
    for reward in rewards {
        let era_start = reward.0.get_start_date_time();
        let month_index = era_start.month0() + (era_start.year() as u32) * 12;
        let acc = *monthly_rewards.get(&month_index).unwrap_or(&0);
        monthly_rewards.insert(month_index, acc + reward.1);
    }
    // fill the missing months
    let min_month_index = *monthly_rewards.keys().min().unwrap();
    let max_month_index = *monthly_rewards.keys().max().unwrap();
    for i in min_month_index..=max_month_index {
        if monthly_rewards.get(&i).is_none() {
            monthly_rewards.insert(i, 0);
        }
    }
    Ok(monthly_rewards)
}

pub fn plot_era_rewards(title: &str, rewards: &[(Era, Balance)]) -> anyhow::Result<PathBuf> {
    let monthly_rewards = get_monthly_rewards(rewards)?;

    let months = vec![
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut domain = vec![];
    let mut data = vec![];
    let mut total = 0;
    for month_index in monthly_rewards.keys().sorted() {
        let month = month_index % 12;
        let year = month_index / 12;
        let reward = *monthly_rewards.get(month_index).unwrap();
        let tick = format!("{} {}", months[month as usize], year % 100);
        domain.push(tick.clone());
        let amount: f32 = format_decimal(
            reward,
            CONFIG.substrate.token_decimals,
            CONFIG.substrate.token_format_decimal_points,
        )
        .parse()
        .unwrap();
        data.push((tick, amount));
        total += reward;
    }
    let total_formatted = format_decimal(
        total,
        CONFIG.substrate.token_decimals,
        CONFIG.substrate.token_format_decimal_points,
    );
    let max = (format_decimal(
        *monthly_rewards.values().max().unwrap(),
        CONFIG.substrate.token_decimals,
        CONFIG.substrate.token_format_decimal_points,
    )
    .parse::<f32>()
    .unwrap()
        * 1.2)
        .ceil();

    let width = 1200;
    let height = 600;
    let (top, right, bottom, left) = (40, 30, 50, 60);
    let x = ScaleBand::new()
        .set_domain(domain)
        .set_range(vec![0, width - left - right])
        .set_inner_padding(0.1)
        .set_outer_padding(0.1);
    let y = ScaleLinear::new()
        .set_domain(vec![0.0, max])
        .set_range(vec![height - top - bottom, 0]);
    let view = VerticalBarView::new()
        .set_x_scale(&x)
        .set_y_scale(&y)
        .set_custom_data_label(format!(
            "Total: {} {}",
            total_formatted, CONFIG.substrate.token_ticker
        ))
        .load_data(&data)
        .unwrap();
    let millis = chrono::Utc::now().timestamp_millis();
    // save svg
    let svg_path = format!(
        "{}{}{}.svg",
        CONFIG.plotter.temp_dir_path,
        std::path::MAIN_SEPARATOR,
        millis,
    );
    if let Err(error) = Chart::new()
        .set_width(width)
        .set_height(height)
        .set_margins(top, right, bottom, left)
        .add_title(title.to_string())
        .add_legend_at(AxisPosition::Top)
        .add_view(&view)
        .add_axis_bottom(&x)
        .add_axis_left(&y)
        .add_left_axis_label(format!("Reward ({})", CONFIG.substrate.token_ticker))
        .set_bottom_axis_tick_label_rotation(-45)
        .save(&svg_path)
    {
        return Err(anyhow::anyhow!("{}", error));
    }

    // save png
    let png_path = format!(
        "{}{}{}.png",
        CONFIG.plotter.temp_dir_path,
        std::path::MAIN_SEPARATOR,
        millis,
    );
    let mut opt = usvg::Options {
        resources_dir: std::fs::canonicalize(&svg_path)
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf())),
        ..Default::default()
    };
    opt.fontdb.load_system_fonts();
    let svg_data = std::fs::read(&svg_path).unwrap();
    let rtree = usvg::Tree::from_data(&svg_data, &opt.to_ref()).unwrap();
    let pixmap_size = rtree.svg_node().size.to_screen_size();
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
    resvg::render(
        &rtree,
        usvg::FitTo::Original,
        tiny_skia::Transform::default(),
        pixmap.as_mut(),
    )
    .unwrap();
    pixmap.save_png(&png_path)?;
    // delete the svg file
    std::fs::remove_file(svg_path)?;
    Ok(PathBuf::from(&png_path))
}