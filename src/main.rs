use chrono::{DateTime, TimeZone, Utc};
use clap::{App, Arg};
use itertools::Itertools;
use plotters::prelude::*;
use rayon::prelude::*;
use regex::Regex;
use serde::Deserialize;
use std::io::{BufReader, Cursor, Read, Write};
use std::ops::Add;
use std::{fs, path::PathBuf};
use tar::Archive;
use time::{ext::NumericalDuration, macros::date, Date, Duration, OffsetDateTime};

#[derive(Debug, Deserialize)]
struct Map {
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct Client {
    pub name: String,
    pub clan: String,
    pub country: i32,
    pub score: i32,
    pub is_player: bool,
}

#[derive(Debug, Deserialize)]
struct Info {
    pub max_clients: i32,
    pub max_players: i32,
    pub passworded: bool,
    pub game_type: String,
    pub name: String,
    pub map: Map,
    pub version: String,
    pub clients: Vec<Client>,
}

#[derive(Debug, Deserialize)]
struct Server {
    pub addresses: Vec<String>,
    pub location: String,
    pub info: Info,
}

#[derive(Debug, Deserialize)]
struct ServerList {
    pub servers: Vec<Server>,
}

fn main() -> anyhow::Result<()> {
    /*let matches = App::new("DDNet Http Master Server Parser")
    .author("Edgar L. <contact@edgarluque.com>")
    .about("Parses the http master server json data to gnuplot format")
    .arg(
        Arg::new("dir")
            .short('d')
            .value_name("DIR")
            .help("The directory with all the json files")
            .takes_value(true)
            .required(true),
    )
    .arg(
        Arg::new("output")
            .short('o')
            .help("The output svg file")
            .takes_value(true)
            .default_missing_value("image.svg"),
    )
    .get_matches();*/

    rayon::ThreadPoolBuilder::new()
        .num_threads(6)
        .build_global()
        .unwrap();

    let mut current_date = date!(2021 - 5 - 18);
    let yesterday = OffsetDateTime::now_utc().date().previous_day().unwrap();

    let mut dates = Vec::with_capacity((yesterday - current_date).whole_days() as usize);

    while current_date <= yesterday {
        dates.push(current_date.clone());
        current_date = current_date.next_day().unwrap();
    }

    dates.into_iter().rev().par_bridge().for_each(|cur_date| {
        create_plot(cur_date).expect("work");
    });
    Ok(())
}

fn create_plot(cur_date: Date) -> anyhow::Result<()> {
    println!("Started processing {}", cur_date);
    let path_regex: Regex =
        Regex::new(r#"(?P<hour>\d{2})_(?P<minute>\d{2})_(?P<second>\d{2}).json"#).unwrap();

    let resp = ureq::get(&format!(
        "https://ddnet.tw/stats/master/{}.tar.zstd",
        cur_date
    ))
    .call()?;

    assert!(resp.has("Content-Length"));
    let len: usize = resp.header("Content-Length").unwrap().parse()?;

    let mut bytes_compressed: Vec<u8> = Vec::with_capacity(len);
    resp.into_reader()
        .take(50_000_000) // read max 50mb
        .read_to_end(&mut bytes_compressed)?;

    let buffer = Cursor::new(bytes_compressed);
    let decoder = zstd::stream::Decoder::new(buffer)?;

    let mut archive = Archive::new(decoder);

    let mut plot_data = archive
        .entries()?
        .step_by((60 * 5) / 5) // There is 1 file every 5 seconds.
        //.step_by((60) / 5) // There is 1 file every 5 seconds.
        .map(|e| {
            let entry = e.unwrap();
            let path = entry.path().unwrap();
            let filename = path.file_name().expect("be a file");
            let filename = filename.to_string_lossy();

            let captures = path_regex.captures(&filename).expect("match regex");

            let hour: u32 = captures.name("hour").unwrap().as_str().parse().unwrap();
            let minute: u32 = captures.name("minute").unwrap().as_str().parse().unwrap();
            let second: u32 = captures.name("second").unwrap().as_str().parse().unwrap();
            //let seconds = (hour * 60 * 60) + (minute * 60) + second;
            let data: ServerList = simd_json::from_reader(entry).expect("parse json");

            let date = chrono::Utc
                .ymd(
                    cur_date.year(),
                    cur_date.month() as u32,
                    cur_date.day() as u32,
                )
                .and_hms(hour, minute, second);
            let total_players = data.servers.iter().flat_map(|x| &x.info.clients).count() as i32;
            let total_players_spectating = data
                .servers
                .iter()
                .flat_map(|x| &x.info.clients)
                .filter(|x| !x.is_player)
                .count() as i32;
            let total_players_playing = data
                .servers
                .iter()
                .flat_map(|x| &x.info.clients)
                .filter(|x| x.is_player)
                .count() as i32;
            (
                date,
                total_players,
                total_players_playing,
                total_players_spectating,
            )
        })
        .collect_vec();

    plot_data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let max_count = plot_data
        .iter()
        .reduce(|a, b| if a.1 > b.1 { a } else { b })
        .unwrap();

    let caption = format!("Master Server Stats on {}", cur_date);

    let file_path = format!("images/{}.svg", cur_date);
    let root_area = SVGBackend::new(&file_path, (1000, 600)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption(&caption, ("sans-serif", 40))
        .build_cartesian_2d(
            chrono::Utc
                .ymd(
                    cur_date.year(),
                    cur_date.month() as u32,
                    cur_date.day().into(),
                )
                .and_hms(0, 0, 0)
                ..plot_data
                    .last()
                    .unwrap()
                    .0
                    .add(chrono::Duration::seconds(1)),
            0..(max_count.1 + 1),
        )
        .unwrap();

    ctx.configure_mesh()
        .x_label_formatter(&|x: &DateTime<Utc>| format!("{}", x.time()))
        .draw()
        .unwrap();

    ctx.draw_series(LineSeries::new(
        plot_data.iter().map(|x| (x.0, x.2)),
        &MAGENTA,
    ))
    .unwrap()
    .label("Players")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &MAGENTA));

    ctx.draw_series(LineSeries::new(plot_data.iter().map(|x| (x.0, x.3)), &RED))
        .unwrap()
        .label("Players Spectating")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

    ctx.draw_series(LineSeries::new(
        plot_data.iter().map(|x| (x.0, x.1)),
        &GREEN,
    ))
    .unwrap()
    .label("Players InGame")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &GREEN));

    ctx.configure_series_labels()
        .position(SeriesLabelPosition::UpperLeft)
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.4))
        .draw()
        .unwrap();

    println!("Finished processing {}", cur_date);
    Ok(())
}
