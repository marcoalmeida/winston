![List & Tests](https://github.com/marcoalmeida/winston/actions/workflows/tests.yml/badge.svg)

# winston
Turn your browser's search bar into a command-line interface

`Winston` is based on `bunny1` which in turn was based on yubnub.org.

For example, typing `g buffy` will open Google's search results on
Buffy (probably the famous vampire slayer).


# Install

## Dependencies
```
apt install rustc
```

## Winston
```
git clone git@github.com:marcoalmeida/winston.git
cd winston
rustup override set nightly
cargo build --release
./target/
```

## Chrome
Set the default search engine to http://localhost:8000/?q=

## Firefox
TBD
