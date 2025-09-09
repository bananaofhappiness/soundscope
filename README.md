# Soundscope — a CLI audio file analyzer tool.
![](https://github.com/bananaofhappiness/soundscope/blob/master/assets/soundscope-demo.gif)
Soundscope is a crossplatform CLI tool for analyzing audio files.

---
## ✨ Features
- 🎤 Analysis of both **audio files** and **microphone input** in **real-time**.
- 📊 **FFT Spectrum** — view the frequency distribution.
- 📉 **Waveform Display** — see the amplitude over time.
- 🔊 **LUFS Metering and True Peak** — measure loudness precisely.

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


---
## 🎥 Demo Video

Watch the demo on [YouTube](https://youtu.be/Z5xJqjMiC1c).

---
## 🐛 Known Issues
- The programm may crash while opening files with length < 15 sec.
- After the playhead reaches the end of the file, the file needs to be reopened in order to be played again. This isn't a bug, but fixing it would be a useful quality‑of‑life (QoL) improvement.
- It works fine with stereo tracks but was not tested with tracks that have different number of channels.

---
## 🛣 Roadmap
- [ ] Zooming the Waveform in and out.
- [ ] Custom themes support.

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
