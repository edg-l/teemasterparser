use rayon::prelude::*;
use regex::Regex;
use serde::Deserialize;
use std::io::{Read, Write};

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
    let matches = App::new("DDNet Http Master Server Parser")
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
        .get_matches();

    let dir_path = matches.value_of("dir").unwrap();
    let output_file = matches.value_of("output").unwrap_or("image.svg");

    let mut paths: Vec<PathBuf> = fs::read_dir(dir_path)?.map(|x| x.unwrap().path()).collect();
    paths.sort();

    let url_regex = Regex::new(r#"(?P<hour>\d{2})_(?P<minute>\d{2})_(?P<second>\d{2}).json"#)?;

    let total = paths.len();
    println!("Processing {} files", total);

    //let mut plot_data = Vec::with_capacity(total);

    let plot_data: Vec<(f64, f64)> = paths
        .par_iter()
        .map(|path| {
            let mut list = Vec::new();
            for mat in url_regex.captures_iter(&path.file_name().unwrap().to_string_lossy()) {
                let hour: f64 = mat.name("hour").unwrap().as_str().parse().unwrap();
                let minute: f64 = mat.name("minute").unwrap().as_str().parse().unwrap();
                let second: f64 = mat.name("second").unwrap().as_str().parse().unwrap();

                let mut file = fs::File::open(path).unwrap();
                let mut buf = String::new();
                file.read_to_string(&mut buf).unwrap();
                let data: ServerList = serde_json::from_str(&buf).unwrap();

                let seconds = (hour * 60.0 * 60.0) + (minute * 60.0) + second;
                let total_players = data
                    .servers
                    .iter()
                    .map(|x| x.info.clients.len())
                    .sum::<usize>() as f64;

                list.push((seconds, total_players));
            }
            list
        })
        .flatten()
        .collect();

    let mut plotter = poloto::plot("Total players", "Time", "Count");
    plotter.line_fill("", &plot_data);
    plotter.xinterval_fmt(|fmt, val, _| {
        let seconds = val % 60.0;
        let minutes = (val / 60.0).floor();
        let hours = (minutes / 60.0).floor();
        let minutes = minutes % 60.0;
        write!(fmt, "{:02}:{:02}:{:02}", hours, minutes, seconds)
    });

    let mut file = fs::File::create(output_file)?;
    write!(file, "{}", poloto::disp(|a| poloto::simple_theme(a, plotter))).unwrap();
    Ok(())
}
