use std::error::Error;

use krpc_client::{client::Client, services::space_center::SpaceCenter};

/// This example creates a stream of get_ut(), awaiting
/// updates at a rate of 1Hz.
fn main() -> Result<(), Box<dyn Error>> {
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

    Ok(())
}
