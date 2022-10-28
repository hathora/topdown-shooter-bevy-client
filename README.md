# Topdown Shooter Bevy Client

An implementation of the multiplayer [Hathora topdown shooter](https://github.com/hathora/topdown-shooter) client using Rust and [Bevy](https://bevyengine.org/). This consumes the [Rust Hathora client](https://github.com/hathora/client-sdk-rs)

Assets from [Kenney](https://kenney.nl/assets/topdown-shooter)

https://user-images.githubusercontent.com/425835/198699248-04438a32-87b9-436d-a088-c5352225a077.mov

## Running Locally

Install cargo via [rustup](https://rustup.rs/)

```
cargo run # create a new room with default APP_ID
cargo run -- $ROOM_ID # connect to an existing room with default APP_ID
cargo run -- $ROOM_ID --app-id $APP_ID # connect to an existing room with a custom APP_ID
```

## Overview

This client reads and writes data from a Hathora server. The server data is treated as authoratitive, so this client just renders server updates and passes inputs to the server for processing.

Server updates and user inputs are sent and received over a websocket. The server is configured to send updates every 50ms. The client update loop is decoupled from server updates by configuring the underlying TCP stream to be non-blocking. When updates are received, they are added to an interpolation buffer. This allows the client to smoothly display updates with a high frame-rate, regardless of the server's tick rate.

## Building a distributable release

`cargo` will generate an executable file. This file assumes that assets like sprites are in specific directories relative to the executable. To build an executable bundled with assets, a release script is available:

```
./release.sh
```
