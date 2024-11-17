# Flatland
Virtual displays for desktop apps

## Support/Questions
Discord: https://discord.gg/PV5CW6Y4SV
Matrix Space: #stardust-xr:matrix.org

## Run
1. Run Monado or WiVRn
2. Run the [Stardust XR server](https://github.com/StardustXR/server/)
3. `cargo run --locked`
4. Launch wayland clients to connect to the stardust server (e.g. `WAYLAND_DISPLAY=wayland-1 <wayland_app>`)

## Usage
### Controllers
Grab a corner handle or both by holding grip on the controller when the cursors are near the handles. Move both corners to a comfortable spot.

Hover over the panel to move the virtual mouse cursor, trigger for left click and A/X for middle click and B/Y for right click.

Touch the panel with a controller to interact with it via multi-touch.

### Hands
Pinch a corner handle or both to move them to a comfortable spot.

Hover over the panel to move the virtual mouse cursor, and pinch for left click. Middle and right click not supported yet.

Directly touch the panel with an index finger or both to use multi-touch.

### Pointers
Unable to resize using pointers but you can still interact with the virtual mouse cursor by hovering over the area and pressing trigger to left click.

### Other
The circle with the symbol on the bottom allows you to grab it and put the window inside the panel shell into another panel shell (e.g. you can move your game to a virtual TV).
The close button works by putting your index finger or controller tip inside it and waiting until it heats up to white hot, then it'll close the window. You have to click and hold for a mouse pointer.

## Todo
- Add corner resize handles for both directions
- Better signifiers for interaction
- Pointer controls for resize/move (keeping in mind this is meant to align to the environment so a grab bar is not ideal, for that see sphereland which is WIP)
