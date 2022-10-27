use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use clap::Parser;
use plotters::prelude::*;
use regex::Regex;
use serde::Deserialize;
use std::{io::BufReader, path::PathBuf, sync::Arc};
use tar::Archive;

#[derive(Debug, Deserialize)]
struct Client {
    pub is_player: bool,
}

#[derive(Debug, Deserialize)]
struct Info {
    pub clients: Option<Vec<Client>>,
}

#[derive(Debug, Deserialize)]
struct Server {
    pub info: Info,
}

#[derive(Debug, Deserialize)]
struct ServerList {
    pub servers: Vec<Server>,
}

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    /// The path to output the svg file. If it doesn't exist outputs to stdout.
    #[arg(short, long)]
    out_path: PathBuf,
    /// Width of the svg image.
    #[arg(short, long, default_value_t = 1920)]
    width: u32,
    /// Height of the svg image.
    #[arg(short, long, default_value_t = 1080)]
    height: u32,
    /// The day to parse. Defaults to yesterday. Format must be %Y-%m-%d
    #[arg(short, long)]
    date: Option<String>,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    let day = {
        if let Some(date) = cli.date {
            NaiveDate::parse_from_str(&date, "%Y-%m-%d")?
        } else {
            Utc::now()
                .checked_sub_signed(chrono::Duration::days(1))
                .unwrap()
                .date_naive()
        }
    };

    create_plot(day, cli.out_path, (cli.width, cli.height))?;

    Ok(())
}

fn create_plot(cur_date: NaiveDate, out_path: PathBuf, size: (u32, u32)) -> color_eyre::Result<()> {
    let path_regex: Regex =
        Regex::new(r#"(?P<hour>\d{2})_(?P<minute>\d{2})_(?P<second>\d{2}).json"#).unwrap();

    let resp = ureq::get(&format!(
        "https://ddnet.org/stats/master/{}.tar.zstd",
        cur_date
    ))
    .call()?;

    let decoder = zstd::stream::Decoder::new(resp.into_reader())?;

    let mut archive = Archive::new(decoder);

    let mut plot_data = archive
        .entries()?
        .step_by(60 / 5) // There is 1 file every 5 seconds and we want to get data every 1 minute.
        .filter_map(|e| e.ok())
        .map(|e| -> color_eyre::Result<_> {
            let path = e.path()?;
            let filename = path.file_name().expect("be a file");
            let filename = filename.to_string_lossy();

            let captures = path_regex.captures(&filename).expect("match regex");

            let hour: u32 = captures.name("hour").unwrap().as_str().parse().unwrap();
            let minute: u32 = captures.name("minute").unwrap().as_str().parse().unwrap();
            let second: u32 = captures.name("second").unwrap().as_str().parse().unwrap();

            let data: ServerList = serde_json::from_reader(BufReader::new(e))?;

            let date = Utc
                .from_local_datetime(&cur_date.and_hms(hour, minute, second))
                .unwrap();

            Ok((date, Arc::new(data)))
        })
        .map(|info| -> color_eyre::Result<_> {
            let (date, data) = info?;
            let total_players = data
                .servers
                .iter()
                .filter_map(|x| x.info.clients.as_ref())
                .flatten()
                .count() as i32;

            let total_players_spectating = data
                .servers
                .iter()
                .filter_map(|x| x.info.clients.as_ref())
                .flatten()
                .filter(|x| !x.is_player)
                .count() as i32;

            let total_players_playing = data
                .servers
                .iter()
                .filter_map(|x| x.info.clients.as_ref())
                .flatten()
                .filter(|x| x.is_player)
                .count() as i32;
            Ok((
                date,
                total_players,
                total_players_playing,
                total_players_spectating,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    plot_data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let max_count = plot_data
        .iter()
        .reduce(|a, b| if a.1 > b.1 { a } else { b })
        .unwrap();

    let caption = format!("Master Server Stats on {}", cur_date);

    let root_area = SVGBackend::new(&out_path, size).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    let from_date = Utc.from_local_datetime(&cur_date.and_hms(0, 0, 0)).unwrap();
    let to_date = plot_data
        .last()
        .unwrap()
        .0
        .checked_add_signed(chrono::Duration::seconds(1))
        .unwrap();

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption(&caption, ("sans-serif", 40))
        .build_cartesian_2d(from_date..to_date, 0..(max_count.1 + 1))?;

    ctx.configure_mesh()
        .x_label_formatter(&|x: &DateTime<Utc>| format!("{}", x.time()))
        .draw()?;

    ctx.draw_series(
        AreaSeries::new(plot_data.iter().map(|x| (x.0, x.1)), 0, &GREEN.mix(0.2))
            .border_style(&GREEN),
    )?
    .label("Players")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &GREEN));

    ctx.draw_series(
        AreaSeries::new(plot_data.iter().map(|x| (x.0, x.2)), 0, &MAGENTA.mix(0.2))
            .border_style(&MAGENTA),
    )?
    .label("Players in game")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &MAGENTA));

    ctx.draw_series(
        AreaSeries::new(plot_data.iter().map(|x| (x.0, x.3)), 0, &RED.mix(0.2)).border_style(&RED),
    )?
    .label("Players Spectating")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

    ctx.configure_series_labels()
        .position(SeriesLabelPosition::UpperLeft)
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.4))
        .draw()?;
    Ok(())
}
