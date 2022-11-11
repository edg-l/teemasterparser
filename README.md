# teemasterparser
[![Version](https://img.shields.io/crates/v/teemasterparser)](https://crates.io/crates/teemasterparser)
[![Downloads](https://img.shields.io/crates/d/teemasterparser)](https://crates.io/crates/teemasterparser)
[![License](https://img.shields.io/crates/l/teemasterparser)](https://crates.io/crates/teemasterparser)
![Rust](https://github.com/edg-l/teemasterparser/workflows/Rust/badge.svg)

Command line tool to parse and analyze data from https://ddnet.tw/stats/master/

# Install
```bash
cargo install teemasterparser
```

## Help
```bash
Usage: teemasterparser [OPTIONS] <COMMAND>

Commands:
  graph      Create graphics
  gamemodes  Game mode related commands
  help       Print this message or the help of the given subcommand(s)

Options:
  -d, --date <DATE>  The day to parse. Defaults to yesterday. Format must be %Y-%m-%d
  -h, --help         Print help information
  -V, --version      Print version information
```

## Run

Run
```bash
teemasterparser -o example.svg
```

## Example Result

![Example image](example.svg "Example")
