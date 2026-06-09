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

The default voice is `af_heart`. The default speed is `1.3`.

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
voice file. hygg passes the id straight through to the model, so any voice
present in that file works. The default is `af_heart`.

Voice ids follow the pattern `<lang><gender>_<name>`. The first letter is the
language/accent (`a` American English, `b` British English, `e` Spanish,
`f` French, `h` Hindi, `i` Italian, `j` Japanese, `p` Brazilian Portuguese,
`z` Mandarin Chinese) and the second is the gender (`f` female, `m` male).

The grade column is Kokoro's own quality grade, from A (best) to F (worst),
based on audio quality and how much data the voice was trained on.

hygg converts text to phonemes with espeak-ng using `en-us` only, so the voice
id changes the speaker's timbre, not the language. The English voices below read
text correctly; the non-English voices are still pronounced as English and are
best treated as experimental accent/timbre options. For the highest quality, try
`af_heart` or `af_bella`.

### American English
| Voice id | Gender | Grade | Notes |
| --- | --- | --- | --- |
| `af_heart` | female | A | default, highest quality ❤️ |
| `af_bella` | female | A- | expressive 🔥 |
| `af_nicole` | female | B- | soft, headphone feel 🎧 |
| `af_aoede` | female | C+ | |
| `af_kore` | female | C+ | |
| `af_sarah` | female | C+ | |
| `af_alloy` | female | C | |
| `af_nova` | female | C | |
| `af_sky` | female | C- | |
| `af_jessica` | female | D | |
| `af_river` | female | D | |
| `am_fenrir` | male | C+ | |
| `am_michael` | male | C+ | |
| `am_puck` | male | C+ | |
| `am_echo` | male | D | |
| `am_eric` | male | D | |
| `am_liam` | male | D | |
| `am_onyx` | male | D | |
| `am_santa` | male | D- | |
| `am_adam` | male | F+ | |

### British English
| Voice id | Gender | Grade |
| --- | --- | --- |
| `bf_emma` | female | B- |
| `bf_isabella` | female | C |
| `bf_alice` | female | D |
| `bf_lily` | female | D |
| `bm_fable` | male | C |
| `bm_george` | male | C |
| `bm_lewis` | male | D+ |
| `bm_daniel` | male | D |

### Other languages
These voices ship in the same file but are still phonemized as `en-us`, so they
apply their speaker's character to English-pronounced text rather than speaking
their native language.

| Voice id | Language | Gender | Grade |
| --- | --- | --- | --- |
| `jf_alpha` | Japanese | female | C+ |
| `jf_gongitsune` | Japanese | female | C |
| `jf_tebukuro` | Japanese | female | C |
| `jf_nezumi` | Japanese | female | C- |
| `jm_kumo` | Japanese | male | C- |
| `zf_xiaobei` | Mandarin Chinese | female | D |
| `zf_xiaoni` | Mandarin Chinese | female | D |
| `zf_xiaoxiao` | Mandarin Chinese | female | D |
| `zf_xiaoyi` | Mandarin Chinese | female | D |
| `zm_yunjian` | Mandarin Chinese | male | D |
| `zm_yunxi` | Mandarin Chinese | male | D |
| `zm_yunxia` | Mandarin Chinese | male | D |
| `zm_yunyang` | Mandarin Chinese | male | D |
| `ef_dora` | Spanish | female | — |
| `em_alex` | Spanish | male | — |
| `em_santa` | Spanish | male | — |
| `ff_siwis` | French | female | B- |
| `hf_alpha` | Hindi | female | C |
| `hf_beta` | Hindi | female | C |
| `hm_omega` | Hindi | male | C |
| `hm_psi` | Hindi | male | C |
| `if_sara` | Italian | female | C |
| `im_nicola` | Italian | male | C |
| `pf_dora` | Brazilian Portuguese | female | — |
| `pm_alex` | Brazilian Portuguese | male | — |
| `pm_santa` | Brazilian Portuguese | male | — |

## Voice blending
Combine voice ids with weights to blend them:
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
