
use std::panic;
use std::fs::OpenOptions;
use std::io::Write;
use std::string::String;
use std;

use backtrace::Backtrace;

pub fn handle_panic(info: &panic::PanicInfo) {
    let trace = Backtrace::new();

    let mut msg = String::new();

    msg.push_str(&format!("\n\n{}\n\n", info));
    msg.push_str(&format!("{:?}", trace));

    write_to_file(&msg).expect(&msg);
}

fn write_to_file(msg: &String) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open("./panic.txt")?;
    write!(file, "{}", msg)?;
    Ok(())
}

