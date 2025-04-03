# Flatland
Virtual displays for desktop apps
> [!IMPORTANT]  
> Requires the [Stardust XR Server](https://github.com/StardustXR/server) to be running. Flatland is required for desktop apps to display correctly.

If you installed the Stardust XR server via:  
```note
sudo dnf group install stardust-xr
```
Or if you installed via the [installation script](https://github.com/cyberneticmelon/usefulscripts/blob/main/stardustxr_setup.sh), Flatland comes pre-installed

## Usage
It's recommended that you use [Hexagon Launcher](https://github.com/StardustXR/protostar), although you can ad an app from the command line via: 
```bash
WAYLAND_DISPLAY=wayland-1 <application> 
```

### Controllers
Grab a corner handle or both by holding grip on the controller when the cursors are near the handles. Move both corners to a comfortable spot.

Hover over the panel to move the virtual mouse cursor, trigger for left click and A/X for middle click and B/Y for right click.

Touch the panel with a controller to interact with it via multi-touch.

### Hands
Pinch a corner handle or both to move them to a comfortable spot.

Hover over the panel to move the virtual mouse cursor, and pinch for left click. Middle and right click not supported yet.

Directly touch the panel with an index finger or both to use multi-touch.

### Mouse Pointer
Unable to resize windows using pointers (will be added in a future update) but you can still interact with the virtual mouse cursor by hovering over the area and pressing trigger to left click.

### Other
The circle with the symbol on the bottom allows you to grab it and put the window inside the panel shell into another panel shell (e.g. you can move your game to a virtual TV).

The close button (X) works by putting your index finger or controller tip inside it and waiting until it heats up to white hot, then it'll close the window. Mouse pointer simply click the X.

## Manual Installation
Clone the repository and after the server is running:
```sh
cargo run
```

## Todo
- Add corner resize handles for both directions
- Better signifiers for interaction
- Pointer controls for resize/move (keeping in mind this is meant to align to the environment so a grab bar is not ideal, for that see sphereland which is WIP)
