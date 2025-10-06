use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    name: String,
}

fn main() {
    let args = Args::parse();
    println!("This is the start of {}!!", args.name);
}
