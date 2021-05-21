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
                .about("The directory with all the json files")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .about("The output file")
                .takes_value(true)
                .default_missing_value("data.dat"),
        )
        .arg(
            Arg::new("output-plot")
                .short('p')
                .about("The output gnuplot file")
                .takes_value(true)
                .default_missing_value("plot.gp"),
        )
        .get_matches();

    let dir_path = matches.value_of("dir").unwrap();
    let output_file = matches.value_of("output").unwrap_or("data.dat");
    let output_plot_file = matches.value_of("output-plot").unwrap_or("plot.pg");

    let mut paths: Vec<PathBuf> = fs::read_dir(dir_path)?.map(|x| x.unwrap().path()).collect();
    paths.sort();

    let url_regex = Regex::new(r#"(?P<hour>\d{2})_(?P<minute>\d{2})_(?P<second>\d{2}).json"#)?;

    let total = paths.len();
    println!("Processing {} files", total);

    let mut plot_data: Vec<String> = paths
        .par_iter()
        .map(|path| {
            let mut list = Vec::new();
            for mat in url_regex.captures_iter(&path.file_name().unwrap().to_string_lossy()) {
                let hour = mat.name("hour").unwrap().as_str();
                let minute = mat.name("minute").unwrap().as_str();
                let second = mat.name("second").unwrap().as_str();

                let mut file = fs::File::open(path).unwrap();
                let mut buf = String::new();
                file.read_to_string(&mut buf).unwrap();
                let data: ServerList = serde_json::from_str(&buf).unwrap();

                let total_players: usize = data.servers.iter().map(|x| x.info.clients.len()).sum();

                list.push(format!(
                    "{}-{}-{} {}\n",
                    hour, minute, second, total_players
                ));
            }
            list
        })
        .flatten()
        .collect();

    let mut file = fs::File::create(output_file)?;
    plot_data.sort();

    for data in &plot_data {
        file.write_all(data.as_bytes())?;
    }

    let mut file = fs::File::create(output_plot_file)?;

    let mut plot = String::new();
    plot.push_str("set xdata time\n");
    plot.push_str("set xlabel 'Day Time'\n");
    plot.push_str("set ylabel 'Concurrent players'\n");
    plot.push_str("set grid\n");
    plot.push_str("set key top left autotitle columnheader\n");
    plot.push_str("set autoscale\n");
    plot.push_str(r#"set timefmt "%H-%M-%S""#);
    plot.push('\n');
    plot.push_str(r#"set format x "%H-%M-%S""#);
    plot.push('\n');
    let first_date = plot_data.first().unwrap().split(' ').next().unwrap();
    let last_date = plot_data.last().unwrap().split(' ').next().unwrap();
    plot.push_str(&format!(r#"set xrange ["{}":"{}"]"#, first_date, last_date));
    plot.push('\n');
    plot.push_str("set terminal png size 1920,1080\n");
    plot.push_str("set output 'data.png'\n");
    plot.push_str("set key top left autotitle columnheader\n");
    plot.push_str(&format!(
        r#"plot "{}" plot "data.dat" using 1:2 smooth csplines lw 2"#,
        output_file
    ));
    plot.push('\n');

    file.write_all(plot.as_bytes())?;

    Ok(())
}
