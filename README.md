# FFprog

FFmpeg with nice progress visualization.

This project is a small tool, mostly for myself, to better visualize the progress of FFmpeg while
it's running. It is specifically built for the goal of re-encoding audio/video with a focus on
reducing the output size.

## Build

Have the latest `rustup`, `rust` toolchain and `cargo` installed and run:

```sh
cargo build
```

## Usage

FFprog works as an overlay over ffmpeg and expects a single input and output file that is not
modified in duration (no skipping, shortening frames or adjusting fps).

A basic command looks like this:

```sh
ffprog -i <input> -- -i <input> ... <several ffmpeg options> ... <output>
```

Everything after the double-dash (`--`) is passed directly to ffmpeg. Only the input file needs
to be given twice, so ffprog can extract some required metadata from it first.

After a successful run a `<input>.stats` file is saved. This contains the data collected during
the ffmpeg run and allows to render the statistics page again without having to run the whole
encoding process again. It can be used as this:

```sh
ffprog -i <input> -s
```

Note that `<input>` is the original file name **without** the `.stats` ending.

### Limitations

This tool is limited in several ways, due to what it was built for, and may not work for every
combination of FFmpeg's plethora of options and knobs.

- Only a single input file is expected, as `ffprog` reads the some metadata from the file at start
  to determine the baseline bitrate and duration.
- Changing the length of the media will result in wrong progress reports as it expects input and
  output to be of the same length.
- Some ffmpeg options may cause ffmpeg to fail as ffprog adds some options on top and ffmpeg might
  not like passing the same or conflicting options.

## License

This project is licensed under the [AGPL-3.0 License](LICENSE) (or
<https://www.gnu.org/licenses/agpl-3.0.html>).
