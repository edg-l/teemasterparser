use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use clap::{Parser, Subcommand};
use plotters::prelude::*;
use regex::Regex;
use serde::Deserialize;
use std::{cmp::Reverse, collections::HashMap, io::Read, path::PathBuf};
use tar::Archive;

#[derive(Debug, Deserialize)]
struct Client {
    pub is_player: bool,
}

#[derive(Debug, Deserialize)]
struct Info {
    pub clients: Option<Vec<Client>>,
    pub game_type: Option<String>,
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
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create graphics.
    Graph {
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
        /// The number of gamemodes to show starting from the most played to the least.
        /// By default it shows the top 10 most famous gamemodes.
        #[arg(short, long, default_value_t = 10)]
        number_gamemodes: usize,
    },
    /// Game mode related commands
    GameModes {
        #[arg(short, long)]
        find: Option<String>,
    },
}

/*
TODO:
- More commands
- Cache?
*/

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Graph {
            out_path,
            width,
            height,
            date,
            number_gamemodes,
        } => {
            let day = {
                if let Some(date) = date {
                    NaiveDate::parse_from_str(&date, "%Y-%m-%d")?
                } else {
                    Utc::now()
                        .checked_sub_signed(chrono::Duration::days(1))
                        .unwrap()
                        .date_naive()
                }
            };

            create_plot(day, out_path, (width, height), number_gamemodes)?;
        }
        Commands::GameModes { find } => {
            todo!()
        }
    };

    Ok(())
}

#[derive(Debug)]
struct PlotData {
    date: DateTime<Utc>,
    total_players: i32,
    total_players_playing: i32,
    total_players_spectating: i32,
    game_types: HashMap<String, usize>,
}

fn create_plot(
    cur_date: NaiveDate,
    out_path: PathBuf,
    size: (u32, u32),
    number_gamemodes: usize,
) -> color_eyre::Result<()> {
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
        .map(|mut e| -> color_eyre::Result<_> {
            let path = e.path()?;
            let filename = path.file_name().expect("be a file");
            let filename = filename.to_string_lossy();

            let captures = path_regex.captures(&filename).expect("match regex");

            let hour: u32 = captures.name("hour").unwrap().as_str().parse().unwrap();
            let minute: u32 = captures.name("minute").unwrap().as_str().parse().unwrap();
            let second: u32 = captures.name("second").unwrap().as_str().parse().unwrap();

            let mut buffer = Vec::with_capacity(e.size() as usize);
            e.read_to_end(&mut buffer)?;
            let data: ServerList = serde_json::from_slice(&buffer)?;

            let date = Utc
                .from_local_datetime(&cur_date.and_hms(hour, minute, second))
                .unwrap();

            Ok((date, data))
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

            let mut game_types = HashMap::new();

            data.servers
                .iter()
                .filter(|x| x.info.clients.is_some())
                .filter(|x| x.info.game_type.is_some())
                .for_each(|x| {
                    let key = x.info.game_type.as_ref().unwrap();
                    let amount = x.info.clients.as_ref().unwrap().len();
                    if let Some(a) = game_types.get_mut(key) {
                        *a += amount;
                    } else {
                        game_types.insert(key.clone(), amount);
                    }
                });

            Ok(PlotData {
                date,
                total_players,
                total_players_playing,
                total_players_spectating,
                game_types,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    plot_data.sort_by(|a, b| a.date.partial_cmp(&b.date).unwrap());

    let max_count = plot_data
        .iter()
        .reduce(|a, b| {
            if a.total_players > b.total_players {
                a
            } else {
                b
            }
        })
        .unwrap();

    let caption = format!("Master Server Stats on {}", cur_date);

    let root_area = SVGBackend::new(&out_path, size).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    let from_date = Utc.from_local_datetime(&cur_date.and_hms(0, 0, 0)).unwrap();
    let to_date = plot_data
        .last()
        .unwrap()
        .date
        .checked_add_signed(chrono::Duration::seconds(1))
        .unwrap();

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption(&caption, ("sans-serif", 60))
        .build_cartesian_2d(from_date..to_date, 0..(max_count.total_players + 1))?;

    ctx.configure_mesh()
        .x_label_formatter(&|x: &DateTime<Utc>| format!("{}", x.time()))
        .x_labels(10)
        .draw()?;

    ctx.draw_series(LineSeries::new(
        plot_data.iter().map(|x| (x.date, x.total_players)),
        &Palette99::pick(0),
    ))?
    .label("Players")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &Palette99::pick(0)));

    ctx.draw_series(LineSeries::new(
        plot_data.iter().map(|x| (x.date, x.total_players_playing)),
        &Palette99::pick(1),
    ))?
    .label("Players in game")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &Palette99::pick(1)));

    ctx.draw_series(LineSeries::new(
        plot_data
            .iter()
            .map(|x| (x.date, x.total_players_spectating)),
        &Palette99::pick(2),
    ))?
    .label("Players Spectating")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &Palette99::pick(2)));

    let mut total_game_types: HashMap<String, usize> = HashMap::new();
    for (game_type, count) in plot_data
        .iter()
        .map(|x| &x.game_types)
        .flat_map(|x| x.iter())
    {
        if let Some(x) = total_game_types.get_mut(game_type) {
            *x += count;
        } else {
            total_game_types.insert(game_type.clone(), *count);
        }
    }

    let mut total_game_types: Vec<(String, usize)> = total_game_types.into_iter().collect();
    total_game_types.sort_by_key(|x| Reverse(x.1));

    // todo show only most famous
    for (idx, (game_type, _)) in total_game_types.iter().enumerate().take(number_gamemodes) {
        let color = Palette99::pick(3 + idx);
        ctx.draw_series(LineSeries::new(
            plot_data.iter().map(|x| {
                (
                    x.date,
                    x.game_types.get(game_type).cloned().unwrap_or(0) as i32,
                )
            }),
            &color,
        ))?
        .label(format!("{} players", game_type))
        .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &color));
    }

    ctx.configure_series_labels()
        .position(SeriesLabelPosition::UpperLeft)
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.4))
        .draw()?;
    Ok(())
}
