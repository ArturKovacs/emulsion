
use std::panic;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::string::String;
use std::iter;
use std::env;

use backtrace::Backtrace;

pub fn handle_panic(info: &panic::PanicInfo) {
    let trace = Backtrace::new();

    let mut msg = String::new();

    let payload = info.payload();
    let payload_string =
        payload.downcast_ref::<&str>().map(|s| *s)
        .or(payload.downcast_ref::<String>().map(|s| s.as_str()));

    msg.push('\n');
    if let Some(panic_message) = payload_string {
        msg.push_str(&format!("\n--\n{}\n--\n\n", panic_message));
    }
    if let Some(location) = info.location() {
        msg.push_str(&format!(
            "Location {}:{}:{}\n\n", location.file(), location.line(), location.column())
        );
    }
    msg.push_str(&format!("{:?}\n", trace));
    for ch in iter::repeat('=').take(99) {
        msg.push(ch);
    }

    eprintln!("\nPanic happened{}", &msg);
    write_to_file(&msg).expect("Could not write panic to file.");
}

fn write_to_file(msg: &String) -> io::Result<()> {
    let curr_exe = env::current_exe()?;
    let curr_exe_dir =
        curr_exe.parent()
        .ok_or(io::Error::new(io::ErrorKind::Other, "Could not get exe parent folder!"))?;

    let mut file =
        OpenOptions::new()
        .create(true)
        .append(true)
        .open(curr_exe_dir.join("panic.txt"))?;

    write!(file, "{}", msg)?;
    Ok(())
}
