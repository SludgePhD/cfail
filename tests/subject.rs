#[test]
fn compiles_and_runs() {
    assert_eq!(1 + 1, 2);
}

#[test]
#[should_panic = "assertion"]
fn compiles_and_panics() {
    assert_eq!(1 + 1, 5);
}

#[test]
#[cfg(compile_fail)]
fn fails_to_compile() {
    let () = 0; //~ E0308
}
