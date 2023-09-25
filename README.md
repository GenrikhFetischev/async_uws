# Rust async HTTP and WebSocket server

Based on  [uWebSockets](https://github.com/uNetworking/uWebSockets), [tokio](https://github.com/tokio-rs/tokio)
and [synchronous rust server](https://github.com/GenrikhFetischev/uwebsockets_rs) (if you are not interested in async
you might find that lib useful)


## Usage
In order to use uWebSockets in your Rust application you will have to link the following static libraries to you
binary - `libz`, `libuv`, `libssl`, `libcrypto` and `libstdc++`.

It may look something like that in your build.rs file:

```rust
println!("cargo:rustc-link-lib=z");
println!("cargo:rustc-link-lib=uv");
println!("cargo:rustc-link-lib=ssl");
println!("cargo:rustc-link-lib=crypto");
println!("cargo:rustc-link-lib=stdc++");
```



