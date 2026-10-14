# Desktop icons

Source: `docs/media/brand/pingloom-mark.svg`, reconstructed from the Gemini logo concept.
The mark is rasterized to 832×832 and centered on a 1024×1024 blush (`#FFF5F5`)
rounded square with 200 px corner radius. `icon.png` is RGBA.

Windows `icon.ico` and macOS `icon.icns` are generated from that PNG with the installed
Tauri CLI (`tauri icon icons/icon.png --output <temporary-directory>`). Only desktop
outputs are retained. Tauri embeds the Windows icon at resource ID 32512; native tray
loading must use the executable module, not the generic system application icon.

Do not use the generated concept-board PNGs as executable icons. Preserve the transparent
weave cuts and the northeast opening; check small-size rendering on physical desktops.
