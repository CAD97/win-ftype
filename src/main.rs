use std::{
    env, io,
    process::{Command, ExitCode},
};
use win_ftype::CommandExt;

fn main() -> io::Result<ExitCode> {
    let args: Vec<_> = env::args().into_iter().skip(1).collect();
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..]);
    let cmd = dbg!(cmd.with_file_type_association()?);
    if { cmd }.status()?.success() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}
