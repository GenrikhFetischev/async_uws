# Rust Async HTTP and WebSocket Server

This server is built on top of `uWebSockets`, leveraging `tokio` for asynchronous runtime support. It is suitable for
Rust applications that require high-performance non-blocking I/O operations. If asynchronous programming is not a
requirement for your project, you may consider using a synchronous Rust server instead.
If you don't need tokio you might be interested
in [libuwebsockets_rs](https://github.com/GenrikhFetischev/uwebsockets_rs) - this is a zero-dependency rust library on
top of `uWebSockets`with the same API as in original package.

## Usage

To integrate `uWebSockets` into your Rust application, you must ensure that several native libraries are correctly
linked to your binary. These libraries are `libz`, `libuv`, `libssl`, `libcrypto`, and the appropriate C++ standard
library for your system.
Configuration for build.rs

Your `build.rs` script should link these libraries like so:

```rs
println!("cargo:rustc-link-lib=z");
println!("cargo:rustc-link-lib=uv");
println!("cargo:rustc-link-lib=ssl");
println!("cargo:rustc-link-lib=crypto");

// Conditional linking for C++ standard library based on the target OS
#[cfg(target_os = "macos")]
println!("cargo:rustc-link-lib=c++"); // Use libc++ for macOS
#[cfg(not(target_os = "macos"))]
println!("cargo:rustc-link-lib=stdc++"); // Use libstdc++ for other systems
```

## Setting Up Your Environment

### macOS Users

On macOS, you might need to specify the include path and library path for the linker to find `libuv` and `libc++`. You
can do this by setting environment variables in your shell:

```sh
export LIBRARY_PATH="/opt/homebrew/lib:$LIBRARY_PATH"
export C_INCLUDE_PATH="/opt/homebrew/include:$C_INCLUDE_PATH"
export CPLUS_INCLUDE_PATH="/opt/homebrew/include:$CPLUS_INCLUDE_PATH"
```

Replace `/opt/homebrew/lib` and `/opt/homebrew/include` with the actual paths if `libuv` is installed in a different
location on your system.

### Linux Users

On Linux, ensure that `libuv`, `libssl`, and `libcrypto` are installed via your distribution's package manager and that
the `libstdc++` library is available on your system.

### Windows Users

Unfortunately this library doesn't currently support Windows, but please don't hesitate sending a PR for Windows support
at [libuwebsockets-sys crate build script](https://github.com/GenrikhFetischev/libuwebsockets-sys/blob/main/build.rs#L13)

## Troubleshooting

If you encounter errors during compilation, such as missing header files (e.g., uv.h not found) or linking issues (e.g.,
library not found for -luv), here are some steps you can follow:

- Verify that all required libraries are installed and accessible in your system's library and include paths.
- Make sure that environment variables are set correctly before running `cargo build`.
- Use `cargo clean` to remove any stale build outputs and then `cargo build` to recompile the project.
- For verbose output that can help with diagnosing build issues, run `cargo build -vv`.

## Contributing

If you encounter any issues or have improvements to suggest, please open an issue or a pull request in the repository.
Your contributions are welcome to help make this project more robust and easier to use for the Rust community.
