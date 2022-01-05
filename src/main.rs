use itertools::Itertools;
use rayon::prelude::*;
use regex::Regex;
use serde::Deserialize;
use std::io::{BufReader, Cursor, Read, Write};
use tar::Archive;
use time::{ext::NumericalDuration, macros::date, Date, Duration, OffsetDateTime};

use clap::{App, Arg};
use std::{fs, path::PathBuf};

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

    while current_date < yesterday {
        dates.push(current_date.clone());
        current_date = current_date.next_day().unwrap();
    }

    dates.into_iter().rev().par_bridge().for_each(|cur_date| {
        create_plot(cur_date).expect("work");
    });
    Ok(())
}

fn create_plot(cur_date: Date) -> anyhow::Result<()> {
    println!("Prcoessing {}", cur_date);
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

    let plot_data = archive
        .entries()?
        .step_by((60 * 5) / 5) // There is 1 file every 5 seconds.
        //.step_by((60) / 5) // There is 1 file every 5 seconds.
        .map(|e| {
            let entry = e.unwrap();
            let path = entry.path().unwrap();
            let filename = path.file_name().expect("be a file");
            let filename = filename.to_string_lossy();

            let captures = path_regex.captures(&filename).expect("match regex");

            let hour: i64 = captures.name("hour").unwrap().as_str().parse().unwrap();
            let minute: i64 = captures.name("minute").unwrap().as_str().parse().unwrap();
            let second: i64 = captures.name("second").unwrap().as_str().parse().unwrap();
            let seconds = (hour * 60 * 60) + (minute * 60) + second;
            let data: ServerList = simd_json::from_reader(entry).expect("parse json");
            let total_players = data
                .servers
                .into_iter()
                .map(|x| x.info.clients.len())
                .sum::<usize>() as f64;
            (seconds as f64, total_players)
        })
        .collect_vec();

    let title = format!("Total players on {}", cur_date);
    let mut plotter = poloto::plot(&title, "Time", "Count");
    plotter.line_fill("", &plot_data);
    plotter.xinterval_fmt(|fmt, val, _| {
        let seconds = val % 60.0;
        let minutes = (val / 60.0).floor();
        let hours = (minutes / 60.0).floor();
        let minutes = minutes % 60.0;
        write!(fmt, "{:02}:{:02}:{:02}", hours, minutes, seconds)
    });

    let mut file = fs::File::create(&format!("images/{}.svg", cur_date))?;
    write!(
        file,
        "{}",
        poloto::disp(|a| poloto::simple_theme(a, plotter))
    )
    .unwrap();
    println!("wrote file");
    Ok(())
}
