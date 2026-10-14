# Asset extraction

`assets/sounds`, `assets/textures` and `assets/emotes` are extracted from a local League of Legends install. This tool regenerates them after a League patch changes the pings or emotes, or when you want to add new emotes to Pingloom / pingloom-rs.

## Requirements

- **Python 3.10+** (Python 3.14 uses built-in `compression.zstd`; older versions use `zstandard`)
- **ffmpeg** on PATH
- **vgmstream-cli** (`vgmstream-cli.exe` from [vgmstream releases](https://github.com/vgmstream/vgmstream/releases))

Install Python dependencies:
```bash
pip install -r tools/extract-assets/requirements.txt
```

Place `vgmstream-cli.exe` on PATH, or drop it into `tools/extract-assets/vgmstream-cli.exe` or `tools/extract-assets/.cache/vgmstream/vgmstream-cli.exe`, or pass `--vgmstream path/to/vgmstream-cli.exe`.

## Emotes

Each emote is baked into an animated WebP (320x320, 3 s, 30 fps, lossy alpha) matching the in-game animation, alongside its PNG icon (`assets/emotes/<slug>.png`) and sound (`assets/emotes/<slug>.ogg`).
When `--emotes` finishes, it automatically updates `EMOTE_ASSETS` in `src/model.rs`.

### 1. Browse and pick emotes (`--emote-gallery`)

```bash
python tools/extract-assets/extract_assets.py --league "C:/Riot Games/League of Legends" --emote-gallery
python -m http.server 8766 --directory "%TEMP%/pingloom-emote-gallery"
```
Open `http://localhost:8766/gallery.html` in your browser.
- Lists ~2,400 emotes from game files.
- You can preview animations, listen to sounds, search by name, and check emotes you want.
- Click "Copy selection" to get the IDs.

### 2. Extract selected emotes (`--emotes`)

Add any new emotes to `EMOTE_SET` in `tools/extract-assets/extract_assets.py` (`'slug': league_id`), then run:

```bash
python tools/extract-assets/extract_assets.py --league "C:/Riot Games/League of Legends" --emotes
```

This will:
- Bake the animated `.webp`, selector `.png`, and sound `.ogg` into `assets/emotes/`.
- Update `EMOTE_ASSETS` in `src/model.rs`.

### 3. Extract ALL emotes (`--all-emotes`)

To extract every valid emote with animation in the game files:

```bash
python tools/extract-assets/extract_assets.py --league "C:/Riot Games/League of Legends" --all-emotes
```

*(Note: Extracting all ~2,400 emotes takes considerable time and disk space).*

## Sounds and Textures

To extract ping sounds:
```bash
python tools/extract-assets/extract_assets.py --league "C:/Riot Games/League of Legends"
```

To extract ping textures from game WADs:
```bash
python tools/extract-assets/extract_assets.py --league "C:/Riot Games/League of Legends" --wad-textures
```


### Developer commands

`emote_bake.py` also has these commands (`--league` is required for all of them):

| Command | What it does |
|---|---|
| `dump --emote <id>` | Prints an emote's effect file and linked templates as JSON |
| `specials` | Lists effects outside the three template families |
| `sounds` | Scans the banks for emote events |
| `bake --emote <id> ...` | Bakes just those emotes into a gallery |

The renderer's assumptions and constants are listed at the top of `emote_bake.py`. One example is `GLOW_OPACITY`, which draws the halo behind an emote at a tenth of its data opacity.
