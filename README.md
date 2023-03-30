# Bevy Tilt Five

This is an initial attempt at a plugin providing support for the [Tilt Five](https://www.tiltfive.com) AR headset for the [Bevy](https://github.com/bevyengine/bevy) game engine.

It is very much a work in progress, and so far is barely functional even in the little functionality that does exist.

Currently, it can render images to multiple pairs of Tilt Five glasses using DX11 (tested up to 4) - but doesn't yet have any wand support.

## Licensing
The library itself is dual licensed under either:

- MIT license
- Apache license

However, the Tilt Five provided files from their SDK are licensed under a separate Apache license, which can be found in [./t5-sdk/readme.txt](./t5-sdk/readme.txt). This covers everything within the t5-sdk folder, as well as the `TiltFiveNative.dll` in the root of the repository.

## Running the example
First - this only supports windows ATM.

To run the example, first clone the repository and make sure you have the most recent version of [Rust](https://www.rust-lang.org/) installed. Once you installed rust and connected your glasses to your computer, you are good to go.

- open a terminal window in repo's directory
- run `cargo run --example simple`
- Once it's done compiling, a window will pop up showing the test scene.
- In the window, in the T5 Status popup, click "Refresh List". A string ID for your connected glasses should show up.
- Click the button for your glasses - you may need to repeat this if it fails to connect.
- Once they are connected, you'll see two previews of what the glasses are showing in the panel, and a button with the ID below it.
- Put on the glasses and test things out.
- When the glasses detect the board, they will output position and rotation information as well.
- To disconnect, click the glasses ID button again.
