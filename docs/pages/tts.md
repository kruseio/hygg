### [<-](../README.md)

## Text to Speech
Text to speech narration is available when hygg is built with the optional
`tts` feature.

Install from crates.io with TTS enabled:
```sh
cargo install --locked --features tts hygg
```

Install from a local checkout with TTS enabled:
```sh
cargo install --locked --path hygg --features tts
```

Start hygg and run `:speak` from inside the reader to narrate from the current
cursor position:
```sh
hygg doc.pdf
```

```text
:speak
:speak stop
```

Any key also stops active narration.

## Voice configuration
The TTS voice and speed are configured with `TTS_VOICE` and `TTS_SPEED`.

Configuration is read in this order:
1. Environment variables for the current process
2. `~/.config/hygg/.env`
3. Built-in defaults

The default voice is `af_sarah`. The default speed is `1.0`.

Use a different voice for one command:
```sh
TTS_VOICE=af_nicole hygg doc.pdf
```

Use a different voice and reading speed:
```sh
TTS_VOICE=af_nicole TTS_SPEED=0.9 hygg doc.pdf
```

Persist the settings:
```sh
mkdir -p ~/.config/hygg
cat >> ~/.config/hygg/.env <<'EOF'
TTS_VOICE=af_nicole
TTS_SPEED=0.9
EOF
```

Environment variables override values in `~/.config/hygg/.env`, which is useful
for trying a voice without changing the saved configuration.

## Voice ids
`TTS_VOICE` must be a Kokoro voice id from the downloaded `voices-v1.0.bin`
voice file. Known examples include:

- `af_sarah`
- `af_nicole`

Voice blending is also supported by combining voice ids with weights:
```sh
TTS_VOICE=af_sarah.4+af_nicole.6 hygg doc.pdf
```

Weights are tenths, so `.4` and `.6` blend the two voices at roughly 40% and
60%.

## Model cache
On first use, hygg downloads the Kokoro ONNX model and voice file into the
platform cache directory under `hygg/tts`.

For offline or air-gapped use, set `HYGG_TTS_MODEL_DIR` to a directory that
already contains:

- `kokoro-v1.0-timestamped.onnx`
- `voices-v1.0.bin`

Example:
```sh
HYGG_TTS_MODEL_DIR=/path/to/hygg-tts-cache hygg doc.pdf
```
