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

fn main() {
    let args = Args::parse();
    println!("This is the start of {}!!", args.name);
    net_hello();
}

fn net_hello() {
    let res = ureq::get("https://jsonplaceholder.typicode.com/posts/1")
        .header("Accept", "application/json")
        .call()
        .unwrap()
        .status()
        .to_string();

    println!("{res}");
}
