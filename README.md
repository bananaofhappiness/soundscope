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
- Open the **e**xplorer by pressing `e`.
- Navigate to your audio file using arrow keys or `h`, `j`, `k`, `l` (Vim-style navigation).
- Press `Enter` to select it.
- Play or pause audio by pressing `Space`.
- Turn **m**id and **s**ide frequencies on/off by pressing `m` and `s` respectively.
- Press `l` to check **l**oudness (LUFS) and `f` to check **f**requencies.
- Use the right and left arrow keys to move playhead 5 seconds forward or backward.
- Alternatively, press `c` to **c**hange input mode from audio file to microphone.
- In microphone mode, choose **d**evice using `d`.
- When you are done, press `q` to **q**uit.
- Change **t**heme by pressing `t`.
- You can also hover over the FFT chart to see additional information about frequency and amplitude. Hold any mouse button to hide this information.

---
## 🎥 Demo Video

Watch the demo on [YouTube](https://youtu.be/AnvWPmRWa1Y?si=9-oT4GtchbH5fqJa).

---
## 🎨 Creating a custom theme
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
borders = "Yellow"
# Frequencies and LUFS tabs text
labels = "Yellow"
axes = "Yellow"
axes_labels = "Yellow"
mid_fft = "Yellow"
side_fft = "170" # Orchid

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
- The application crashes when trying to select a microphone with low sample rate (at least ≤16000).

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

If you like **soundscope** and want to support its development:

- Ethereum (ERC-20): 0xe8f2dd8a2d3a6ba9c571aadc720b6f1bea47fe4a
- [Boosty](https://boosty.to/bananaofhappiness)

Your support helps keep this project alive ❤️
