# Rust Paper

A Rust-based wallpaper manager for Linux/UNIX systems that fetches wallpapers from [Wallhaven](https://wallhaven.cc/).

## Installation

To get started with `rust-paper`, first install it:

```bash
cargo install rust-paper
```

## Configuration

Run `rust-paper` once to initialize config directory and files.

Configuration files are stored in different locations depending on your operating system:

- **Linux:** `~/.config/rust-paper/config.toml`
- **macOS:** `~/Library/Application Support/rs.rust-paper/config.toml`

### Example `config.toml`

```toml
save_location = "/Users/abhaythakur/Pictures/wall"
integrity = true
api_key = "your_wallhaven_api_key_here"
max_concurrent_downloads = 10
timeout = 30
retry_count = 3
```

#### Configuration Options:

- `save_location`: The directory where wallpapers will be saved
- `integrity`: If set to `true`, SHA256 checksums will be used for integrity verification
- `api_key` (optional): Wallhaven API key for higher rate limits and access to new features
- `max_concurrent_downloads`: Maximum number of simultaneous downloads (default: 10)
- `timeout`: HTTP request timeout in seconds (default: 30)
- `retry_count`: Number of retry attempts for failed requests (default: 3)

### Additional Files

- `wallpaper.lock`: This file is used for integrity checks when `integrity` is set to `true`.
- `wallpapers.lst`: This file stores the IDs of the wallpapers from Wallhaven. An example of its content is shown below:

```plaintext
p9pzk9
x6m3gl
gpl8d3
5gqmg7
qzp8dr
yx3kok
85pgqk
3lgk6y
kx6yqm
o5ww39
o5m9xm
l8rloq
l8o2op
7pmgv9
```

## API Key Setup (Optional but Recommended)

To use advanced features like search, user settings, and collections, you need a Wallhaven API key:

1. Visit [Wallhaven.cc](https://wallhaven.cc/) and create an account
2. Go to Settings → API and generate your API key
3. Add it to your config file or set as environment variable:

```bash
export WALLHAVEN_API_KEY="your_api_key_here"
```

## Usage

Once configured, you can run the application to download and manage wallpapers seamlessly.

### Command Line Interface

```bash
rust-paper <COMMAND>
```

#### Basic Commands (No API Key Required):

- **`sync`** - Sync all wallpapers in your list
```bash
rust-paper sync
```

- **`add`** - Add new wallpapers to your list
```bash
rust-paper add 7pmgv9,l8o2op
# Or
rust-paper add 7pmgv9 l8o2op
# Or with URLs
rust-paper add https://wallhaven.cc/w/7pmgv9 https://wallhaven.cc/w/l8o2op
```

- **`remove`** - Remove wallpapers from your list
```bash
rust-paper remove 7pmgv9 l8o2op
```

- **`list`** - List all tracked wallpapers with download status
```bash
rust-paper list
```

- **`clean`** - Remove downloaded wallpapers not in your list
```bash
rust-paper clean
```

- **`info`** - Show detailed information about a wallpaper (works with or without API key)
```bash
rust-paper info 7pmgv9
```

#### Advanced Commands (Require API Key):

- **`search`** - Search and download wallpapers by query or color
```bash
# Search by query
rust-paper search --query "anime +city" --download

# Search by color
rust-paper search --colors 722f37 --download

# Random wallpaper
rust-paper search --query "" --sorting RANDOM --download
```

**Note:** The `--download` flag saves wallpapers to `save_location` from your config, using the Wallhaven ID as the filename.

- **`tag-info`** - Get tag information
```bash
rust-paper tag-info 15
```

- **`user-settings`** - Show your Wallhaven account settings
```bash
rust-paper user-settings
```

- **`user-collections`** - Show user collections
```bash
rust-paper user-collections
```

- **`help`** - Print help message
```bash
rust-paper help
```

For detailed information about API features, see [API_INTEGRATION.md](API_INTEGRATION.md).

#### Options:

- `-h, --help` Print help

## Contributing

Contributions are welcome! Feel free to submit issues or pull requests.

## <span>🤝 Thanks ❤️</span>

- [dax99993/wallhaven](https://github.com/dax99993/wallhaven)

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for more details.

---
