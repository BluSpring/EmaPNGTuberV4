# EmaPNGTuberV4

A custom PNGTuber software primarily made for [EmaMae](https://twitch.tv/IAmEmaMae).
What happened to the other 3 versions? *no.*

## Why?
I saw Ema have performance issues with the PNGTuber software she was using.
And of course, with the way I am, I just decided to make my own version for them.

This PNGTuber software seems to have less than 1-2% CPU *and* GPU usage on my computer, and
less than 90 MB of memory used, though it could potentially be better.

It utilizes a dynamic rendering feature, where the renderer stops rendering if no updates
are required. This highly helps with the resource usage, especially when idle.

## Usage
You can right-click on the window to toggle the frame on/off, and if the frame is
on, it will show the properties button in the top right.

If the PNGTuber avatar is invisible, make sure the threshold is at -30.0 dB.

## Building
In order to build the project, you must first install the .dll and .lib files required by SDL2.
<br>
You can get the files here: [SDL2](https://github.com/libsdl-org/SDL/releases/tag/release-2.30.0) [SDL2_ttf](https://github.com/libsdl-org/SDL_ttf/releases/tag/release-2.22.0) [SDL2_image](https://github.com/libsdl-org/SDL_image/releases/tag/release-2.8.2)

This project was compiled and tested on Windows, but it *might* work on other platforms too with
some slight modifications.