// This test does not use a harness so that the output doesn't get captured, makes debugging easier.

fn main() -> cfail::Result<()> {
    cfail::Config::new()?.exclude_dir("src")?.run_tests()
}
