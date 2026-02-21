# Soundscope — a CLI audio file analyzer tool.
![](https://github.com/bananaofhappiness/soundscope/blob/master/assets/soundscope-demo.gif)
Soundscope is a crossplatform CLI tool for analyzing audio files.

---
## ✨ Features
- 🎤 Analysis of both **audio files** and **microphone input** in **real-time**.
- 📊 **FFT Spectrum** — view the frequency distribution.
- 📉 **Waveform Display** — see the amplitude over time with Min-Max Decimation algorithm.
- 🔊 **LUFS Metering and True Peak** — measure loudness precisely.
- 🎨 **Customizable Theme** — change the color scheme to your liking.

## 🚀 Installation

### Using pacman (on Arch Linux)

```
sudo pacman -S soundscope
```

### Using Cargo

```
cargo install soundscope
```
or
```
cargo install --git https://github.com/bananaofhappiness/soundscope
```

### Precompiled Binaries

Grab the latest release for your platform from the [**Releases page**](https://github.com/bananaofhappiness/soundscope/releases).

---
## 🔧 Usage
- Run the tool using `soundscope` command. You can optionally provide an audio file path to open it directly on startup:
  ```
  soundscope path/to/audio.mp3
  ```
- Press `h`, `?`, or `F1` to view the help popup with all available keyboard shortcuts.

---
## 🎨 Themes

### Built-in Themes
Soundscope comes with several pre-built themes that you can use directly without creating a custom theme file:
- **Catppuccin** — Mocha, Macchiato, Frappé, Latte
- **Dracula** — Classic dark theme with vibrant accents
- **Gruvbox Dark** — Warm, retro color scheme
- **Material Dark** — Based on Google's Material Design
- **Monokai** — Classic dark theme with high contrast
- **Nord** — Arctic, north-bluish color palette
- **One Dark** / **One Light** — Popular themes from Atom/VSCode
- **Solarized** — Dark and Light variants with precision colors
- **Tokyo Night** — Inspired by Tokyo's neon nightlife
- **Ayu Dark** — Bright colors comfortable for all-day use
- **Black & White** / **White & Black** — Minimal monochrome themes

Press `t` in the application to open the theme selection list and choose any of these built-in themes.

### Creating a custom theme
The theme is set in `.theme` file which must be placed in `{YOUR_CONFIG_DIRECTORY}/soundscope` directory. Under the hood it is a simple `.toml` file. Here is an example theme (which is default for the app) containing all possible variables:
```toml
[global]
background = "Black"
# It is default value for everything that is not a background
foreground = "221" # It is an ANSI-256 value for LightGoldenrod2 color. See https://www.ditig.com/256-colors-cheat-sheet.

# Color used to highlight corresponding characters
# Like highlighting L in LUFS to let the user know
# that pressing L will open the LUFS meter
highlight = "160" # Red3 color. Note that it can also be written as "#d70000"

# For simplicity yellow color in this example is written as "Yellow" instead of "221", and light red is written as "LightRed" instead of "160". But default color scheme uses LightGoldenrod2 for foreground and Red3 for highlight.
[waveform]
borders = "Yellow"
waveform = "Yellow"
playhead = "LightRed"# if not set, default is highlighted color
# Current playing time and total duration
current_time = "Yellow"
total_duration = "Yellow"
# Buttons like <-, +, -, ->
controls = "Yellow"
# Color of a button when it's pressed
controls_highlight = "LightRed"
labels = "Yellow"

[fft]
foreground = "Yellow"
background = "Black"
borders = "Yellow"
# Frequencies and LUFS tabs text
labels = "Yellow"
axes = "Yellow"
axes_labels = "Yellow"
mid_fft = "Yellow"
side_fft = "LightRed"

[lufs]
axis = "Yellow"
chart = "Yellow"
# Frequencies and LUFS tabs text
labels = "Yellow"
# Text color on the left
foreground = "Yellow"
# Color of the numbers on the left
numbers = "Yellow"
borders = "Yellow"
background = "Black"
highlight = "LightRed"

[devices]
background = "Black"
foreground = "Yellow"
borders = "Yellow"
highlight = "LightRed"

[explorer]
background = "Black"
borders = "Yellow"
item_foreground = "Yellow"
highlight_item_foreground = "LightRed"
dir_foreground = "Yellow"
highlight_dir_foreground = "LightRed"

[error]
background = "Black"
foreground = "LightRed"
borders = "LightRed"

[help]
background = "Black"
foreground = "Yellow"
borders = "Yellow"
highlight = "LightRed"
```

Only global foreground and global background colors are mandatory. You can pass the ANSI-256 color number (see [this cheat sheet](https://www.ditig.com/256-colors-cheat-sheet)) or HEX color code (prefixed with `#`) or use one of the predefined colors below:
```
- Black
- Red
- Green
- Yellow
- Blue
- Magenta
- Cyan
- Gray
- DarkGray
- LightRed
- LightGreen
- LightYellow
- LightBlue
- LightMagenta
- LightCyan
- White
- Reset
```
`Reset` restores the terminal's default color. This can be useful if you're using a transparent background.

Color separators `-`, `_`, and ` ` are supported and names are case insensitive. For example, `Light-blue` or `light_blue` or `light Blue` are all valid.

After saving your theme into `.theme` file and placing it into `{YOUR_CONFIG_DIRECTORY}/soundscope`, press `t` to open up the theme selection list and choose yours.

---
## 🐛 Known Issues

**Note:** In some audio file formats (particularly `.m4a`, `.mp3`), the playhead may gradually drift slightly to the right of the waveform center over time. This is a known behavior in the [Symphonia](https://github.com/pdeljanov/Symphonia) library for formats that are not gapless. This does not affect the visualization itself.

Other known issues:
- Rapidly seeking through an audio file may cause lag, resulting in the playhead being in an incorrect position. Pausing playback and waiting for the playhead to return to the correct spot before resuming usually resolves the issue.

---
## 🛣 Roadmap
- [x] Microphone input. ([v1.1.0](https://github.com/bananaofhappiness/soundscope/releases/tag/v1.1.0), 2025-09-09)
- [x] Zooming the Waveform in and out. ([v1.4.0](https://github.com/bananaofhappiness/soundscope/releases/tag/v1.4.0), 2025-10-06)
- [x] Custom themes support. ([v1.2.0](https://github.com/bananaofhappiness/soundscope/releases/tag/v1.2.0), 2025-19-09)
- [x] Frequency and amplitude details on hover over the FFT chart. ([v1.5.0](https://github.com/bananaofhappiness/soundscope/releases/tag/v1.5.0), 2025-12-15)
- [x] Command-line argument support for opening audio files directly. ([v1.6.0](https://github.com/bananaofhappiness/soundscope/releases/tag/v1.6.0), 2026-01-24)
- [ ] Real-time compression with the ability to see applied changes in the waveform in real-time.

---
## 🤝 Contributing

Pull Requests, Issues and Suggestions are welcome!

---
## 📜 License

This project is licensed under the **MIT License** — see [LICENSE](LICENSE) for details.

---
## ☕ Support

If you like **Soundscope** and want to support its development:

- Ethereum (ERC-20): 0xe8f2dd8a2d3a6ba9c571aadc720b6f1bea47fe4a
- Bitcoin: bc1qype09urnpfztgrw6af83a2g86jrfhf5tr8dwp8
- Solana: 9YmZXS7uYJSY9AwLmVmbDTgAAp987W3mQ1P3U5MUQ9Sv
- Tron: TSwN6uW67KrmwvGUmXjJVAykmaWQ5wRZnM
- [Boosty](https://boosty.to/bananaofhappiness)

Your support helps keep this project alive ❤️
