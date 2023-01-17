# Bevy Tilt Five

This is an initial attempt at a plugin providing support for the [Tilt Five](https://www.tiltfive.com) AR headset for the [Bevy](https://github.com/bevyengine/bevy) game engine.

It is very much a work in progress, and so far is barely functional even in the little functionality that does exist.

Currently, it can render images to a single pair of Tilt Five glasses using DX11. The tracking of the headset is currently still broken, and no thought has gone into implementing the wand input yet. However, the goal is to provide full support for multiple headsets & their inputs.


## Licensing
The library itself is dual licensed under either:

- MIT license
- Apache license

However, the Tilt Five provided files from their SDK are licensed under a separate Apache license, which can be found in [./t5-sdk/readme.txt](./t5-sdk/readme.txt). This covers everything within the t5-sdk folder, as well as the `TiltFiveNative.dll` in the root of the repository.

