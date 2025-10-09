# Soundscope — a CLI audio file analyzer tool.
![](https://github.com/bananaofhappiness/soundscope/blob/master/assets/soundscope-demo.gif)
Soundscope is a crossplatform CLI tool for analyzing audio files.

---
## ✨ Features
- 🎤 Analysis of both **audio files** and **microphone input** in **real-time**.
- 📊 **FFT Spectrum** — view the frequency distribution.
- 📉 **Waveform Display** — see the amplitude over time.
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
- Run the tool using `soundscope` command
- Open the **e**xplorer by pressing `E`.
- Navigate to your audio file using arrow keys or `H`, `J`, `K`, `L` (Vim-style navigation).
- Press `Enter` to select it.
- Play or pause audio by pressing `Space`.
- Turn **M**id and **S**ide Frequencies on/of by pressing `M` and `S` respectively.
- Press `L` to check **L**UFS and `F` to check **f**requencies.
- Use the right and left arrow keys to move playhead 5 seconds forward or backward.
- Alternatively, press `C` to **C**hange input mode from audio file to microphone.
- In microphone mode, choose **D**evice using `D`.
- When you are done, press `Q` to **q**uit.
- Change **T**heme by pressing `T`.

---
## 🎥 Demo Video

Watch the demo on [YouTube](https://youtu.be/Z5xJqjMiC1c).

---
## 🎨 Creating a custom theme
The theme is set in `.theme` file which must be placed in `{YOUR_CONFIG_DIRECTORY}/soundscope` directory. Under the hood it is a simple `.toml` file. Here is an example theme (which is default for the app) containing all possible variables:
```toml
[global]
background = "Black"
# It is default value for everything that is not a background
foreground = "Yellow"
# Color used to highlight corresponding characters
# Like highlighting L in LUFS to let the user know
# that pressing L will open the LUFS meter
highlight = "LightRed"

[waveform]
borders = "Yellow"
waveform = "Yellow"
playhead = "LightRed "# if not set, default is LightRed
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
side_fft = "LightGreen"

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
foreground = "Yellow"
borders = "Yellow"
```

Only global foreground and global background colors are mandatory. You can pass the HEX color code or use one of the predefined colors below:
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
```
Color separators `-`, `_`, and ` ` are supported and names are case insensitive. For example, `Light-blue` or `light_blue` or `light Blue` are all valid.
After saving your theme into `.theme` file and placing it into `{YOUR_CONFIG_DIRECTORY}/soundscope`, press `T` to open up the theme selection list and choose yours.

---
## 🐛 Known Issues
- After the playhead reaches the end of the file, the file needs to be reopened in order to be played again. This isn't a bug, but fixing it would be a useful quality‑of‑life (QoL) improvement.
- It works fine with stereo tracks but was not tested with tracks that have different number of channels.

---
## 🛣 Roadmap
- [x] Microphone input.
- [x] Zooming the Waveform in and out.
- [x] Custom themes support.

---
## 🤝 Contributing

Pull Requests, Issues and Suggestions are welcome!

---
## 📜 License

This project is licensed under the **MIT License** — see [LICENSE](LICENSE) for details.

---
## ☕ Support

If you like **soundscope** and want to support its development:

- [Boosty](https://boosty.to/bananaofhappiness)

Your support helps keep this project alive ❤️
