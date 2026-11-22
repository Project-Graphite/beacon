use std::io::Write;

fn main() {
    let home = std::env::var_os("BEACON_HOME").expect("BEACON_HOME is set by the test");
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut launches = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::path::Path::new(&home).join("launches"))
        .expect("the launch log opens");
    writeln!(launches, "{}", arguments.join(" ")).expect("the launch is logged");
}
