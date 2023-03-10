# kRPC Client

Rust client for [kRPC](https://github.com/krpc/krpc) (Remote Procedure Calls for Kerbal Space Program).

### Disclaimer

Work in progress, some procedures may not work. Bug-reports and contributions welcome.

### Example

Orient a craft along its prograde vector:

```rust
// Create a new kRPC client.
let client = Arc::new(
    Client::new("rpc test", "127.0.0.1", 50000, 50001).unwrap(),
);

// Initialize the SpaceCenter service.
let sc = SpaceCenter::new(Arc::clone(&client));

// Call procedures.
let ship = sc.get_active_vessel()?;
let ap = ship.get_auto_pilot()?;

let svrf = ship.get_orbital_reference_frame()?;
let aprf = ap.get_reference_frame()?;

let direction =
    sc.transform_direction((0.0, 1.0, 0.0), &svrf, &aprf)?;

ap.set_target_direction(direction)?;
ap.engage()?;
ap.wait()?;
ap.disengage()?;
```

### Streams

Not yet.

### Hacking

* `krpc-client/client.rs` contains basic connection, request, and response handling.
* `krpc-client/lib.rs` declares traits for encoding and decoding RPC types.
* `krpc-build` (used by `krpc-client/build.rs`), generates RPC types and procedures from definitions in `service_definitions/*.json`, and generates implementations of the encoding and decoding traits declared in `krpc-client/lib.rs`.
