use std::error::Error;

use krpc_client::{client::Client, services::space_center::SpaceCenter};

/// This example shows basic usage of the client and its
/// types.
fn main() -> Result<(), Box<dyn Error>> {
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

    Ok(())
}
