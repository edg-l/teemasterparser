# teemasterparser

Command line tool to parse and analyze data from https://ddnet.tw/stats/master/

# Install
```bash
cargo install teemasterparser
```

## Help
```bash
$ teemasterparser --help
Parses the data from one day in https://ddnet.tw/stats/master/ and outputs a SVG plot with total players.

Usage: teemasterparser.exe <COMMAND>

Commands:
  graph       Create graphics
  game-modes  Game mode related commands
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```

## Run

Run
```bash
teemasterparser -o example.svg
```

## Example Result

![Example image](example.svg "Example")