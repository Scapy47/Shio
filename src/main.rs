use clap::Parser;
// use serde::Deserialize;
use ureq;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    name: String,
}

// #[derive(Deserialize)]
// struct MyRecvBody {
//     #[serde(rename = "userId")]
//     user_id: u32,
//     id: u32,
//     title: String,
//     body: String,
// }

fn main() -> Result<(), ureq::Error> {
    let args = Args::parse();
    println!("This is the start of {}!!", args.name);
    net_hello()?;
    Ok(())
}

fn net_hello() -> Result<(), ureq::Error> {
    let res = ureq::get("https://jsonplaceholder.typicode.com/posts/")
        .header("Accept", "application/json")
        .call()?
        .body_mut()
        .read_to_string()?;

    println!("{res}");
    Ok(())
}
