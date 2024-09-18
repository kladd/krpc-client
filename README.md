# kRPC Client

Rust client for [kRPC](https://github.com/krpc/krpc) (Remote Procedure Calls for Kerbal Space Program).

### Status

Work in progress. Bug-reports and contributions welcome. All procedures seem to work, but more testing is needed. Streams work, but Events are still on the way.

```toml
krpc-client = { git = "https://github.com/kladd/krpc-client" }
```

### Examples

Greet the crew with standard procedure calls.

```rust
let client = Client::new("kRPC TEST", "127.0.0.1", 50000, 50001).unwrap();

let sc = SpaceCenter::new(client.clone());

// Check out our vessel.
let ship = sc.get_active_vessel()?;

// Greet the crew.
match ship.get_crew()?.first() {
    Some(kerbal) => println!(
        "Hello, {}. Welcome to {}",
        kerbal.get_name()?,
        ship.get_name()?
    ),
    None => println!("{} is unkerbaled!", ship.get_name()?),
};
```

### Using Streams

Keep track of time with streams.

```rust
let client = Client::new("kRPC TEST", "127.0.0.1", 50000, 50001)?;

let space_center = SpaceCenter::new(client.clone());

// Set up a stream.
let ut_stream = space_center.get_ut_stream()?;
ut_stream.set_rate(1f32)?;

// Wait for updates, and print the current value.
for _ in 0..10 {
    ut_stream.wait();
    println!("It's {} o'clock", ut_stream.get()?);
}
```

### Features
* `fmt` (default): Format generated services. Remove for a quicker build producing an unreadable file.
* `tokio`: Replace all blocking functions with async functions using the tokio runtime

### Hacking

* `krpc-client/client.rs` contains basic connection, request, and response handling.
* `krpc-client/lib.rs` declares traits for encoding and decoding RPC types.
* `krpc_build` (used by `krpc-client/build.rs`), generates RPC types and procedures from definitions in `service_definitions/*.json`, and generates implementations of the encoding and decoding traits declared in `krpc-client/lib.rs`.
