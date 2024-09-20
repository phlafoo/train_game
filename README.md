## Train Game (tentative title)

**Warning - Very WIP**

The goal is to not be touched by the enemies. Gamepad controller is highly recommended.

To run:
```
cargo run --release -- -m [level filename]
```
Level file is assumed to in `assets/levels/`. For example:
```
cargo run --release -- -m test.tmx
```

You can used the [Tiled](https://www.mapeditor.org/) map editor to edit the existing levels or create your own. You must use the `assets/tilesets/tileset16x.tsx` tileset.

### Controls
###### Keyboard
```
Arrows/WASD -  movement
LShift      -  hold for boost

C           -  show/hide game config
X           -  show/hide bevy world inspector
V           -  toggle V-sync
N           -  toggle noclip
R           -  reset level
Plus        -  zoom camera in
Minus       -  zoom camera out
LControl    -  hold to zoom camera faster
0           -  reset camera
```
###### Gamepad
```
Left stick  -  movement
X/A         -  hold for boost
```
