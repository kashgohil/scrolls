# scrolls

```
███████╗ ██████╗██████╗  ██████╗ ██╗     ██╗     ███████╗
██╔════╝██╔════╝██╔══██╗██╔═══██╗██║     ██║     ██╔════╝
███████╗██║     ██████╔╝██║   ██║██║     ██║     ███████╗
╚════██║██║     ██╔══██╗██║   ██║██║     ██║     ╚════██║
███████║╚██████╗██║  ██║╚██████╔╝███████╗███████╗███████║
╚══════╝ ╚═════╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚══════╝╚══════╝
                  your terminal scrolls
```

A terminal RSS reader built with [ratatui]. Browse, cache, and read your
feeds without leaving the terminal.

## Features

- Three-pane home: categories, feeds, and an in-line article list
- In-app reader that renders feed HTML as styled text
- SQLite-backed cache — articles and read state survive restarts
- OPML import and export
- Background fetching with periodic refresh
- Open the original article in your system browser with one key
- Toasts for fetch errors and import/export results

## Installation

**macOS / Linux — no Rust required.** Downloads a prebuilt binary into
`~/.local/bin` (make sure that's on your `PATH`):

```sh
curl -fsSL https://raw.githubusercontent.com/kashgohil/scrolls/master/install.sh | sh
```

**With Rust** (any platform):

```sh
cargo install --git https://github.com/kashgohil/scrolls
```

The first run seeds the database with a couple of Rust blog feeds. After
that, anything you've subscribed to is loaded from the cache on startup.

## Usage

Just run `scrolls` and start reading. The keybindings are shown on each
view's block; here's the full set:

| Key              | Home (categories / feeds)           | Articles                          | Reader                              |
| ---------------- | ----------------------------------- | --------------------------------- | ----------------------------------- |
| `↑` / `↓`        | move highlight                      | move highlight                    | scroll line by line                 |
| `→` / `Enter`    | move into feeds / open feed         | open article in reader            | next article                        |
| `←`              | back to categories                  | back to home                      | previous article                    |
| `a`              | add a feed (URL, then category)     | —                                 | —                                   |
| `c`              | set category on selected feed       | —                                 | —                                   |
| `d`              | delete selected feed                | —                                 | —                                   |
| `i` / `e`        | import / export OPML                | —                                 | —                                   |
| `r`              | refresh all feeds                   | —                                 | —                                   |
| `A`              | —                                   | mark all in feed as read          | —                                   |
| `o`              | —                                   | open article in browser           | open article in browser             |
| `Esc`            | back to categories                  | back to home                      | back to articles                    |
| `q`              | quit                                | quit                              | quit                                |

## Data

Scrolls keeps its database and OPML exports under your platform's data
directory (the `dirs` crate's `data_dir`):

- `scrolls/scrolls.db` — feeds, cached articles, and read state
- `scrolls/feeds.opml` — last OPML export (created on first export)

## License

MIT.

[ratatui]: https://ratatui.rs
[Rust toolchain]: https://rustup.rs
