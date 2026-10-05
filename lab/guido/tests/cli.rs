use std::{
    fs,
    io::{BufReader, Cursor},
    process::Command,
};

#[test]
fn command_writes_native_srgb_png_and_preserves_existing_output() {
    let output =
        std::env::temp_dir().join(format!("resina-material-board-{}.png", std::process::id()));
    let args = [
        "material-board".to_owned(),
        std::env::var("RESINA_SCENES").expect("RESINA_SCENES required"),
        std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT required"),
        "DejaVu Sans".to_owned(),
        output.to_str().unwrap().to_owned(),
        "1.25".to_owned(),
        "1".to_owned(),
    ];
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_resina-lab"))
            .args(&args)
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(&output).unwrap();
    let mut reader = png::Decoder::new(BufReader::new(Cursor::new(&bytes)))
        .read_info()
        .unwrap();
    assert_eq!((reader.info().width, reader.info().height), (1260, 1210));
    assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    assert!(reader.info().srgb.is_some());
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(frame.buffer_size(), 1260 * 1210 * 4);
    assert!(
        pixels[..frame.buffer_size()]
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[3] == 255)
    );
    for (index, diagnostic) in [(1, "scene catalog"), (2, "font file")] {
        let mut missing = args.clone();
        missing[index] = output
            .with_extension("missing")
            .to_str()
            .unwrap()
            .to_owned();
        let result = Command::new(env!("CARGO_BIN_EXE_resina-lab"))
            .args(&missing)
            .output()
            .unwrap();
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains(diagnostic) && error.contains(&missing[index]));
        assert_eq!(fs::read(&output).unwrap(), bytes);
    }
    let existing = run();
    assert!(!existing.status.success());
    assert_eq!(fs::read(&output).unwrap(), bytes);
    fs::remove_file(output).unwrap();
    let bad = Command::new(env!("CARGO_BIN_EXE_resina-lab"))
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("usage:"));
}
