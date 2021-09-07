# A Minimal X11 Window Manager in Rust

This is a project I am creating for fun. The goal is to have a functional tiling window manager.

## Getting Started

### Dependencies

* An X11 server like [Xorg](https://wiki.archlinux.org/title/Xorg)
* Rust and Cargo

### Running

* Make sure you aren't running any other window manager
* In the project directory run `cargo build --release`
* Run the executable created in `target/release`

## License

This project is licensed under the GPLv2 license. See [LICENSE](LICENSE).

## Acknowledgements

Here are a few sources I used to learn how to make this:

* [X Window System Protocol](https://www.x.org/releases/X11R7.7/doc/xproto/x11protocol.html)
* [The Xlib Manual](https://tronche.com/gui/x/xlib/)
* [How X Window Managers Work, And How To Write One](https://jichu4n.com/posts/how-x-window-managers-work-and-how-to-write-one-part-i/)
* [DWM source code](https://git.suckless.org/dwm/files.html)
* [lanta](https://github.com/mjkillough/lanta)
