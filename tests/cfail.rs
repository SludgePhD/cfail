#[test]
fn cfail() -> cfail::Result<()> {
    cfail::Config::new()?.exclude_dir("src")?.run_tests()
}
