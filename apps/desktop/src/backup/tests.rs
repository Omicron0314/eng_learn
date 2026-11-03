use super::*;
#[test]
fn audio_blobs_are_sealed_written_and_rejected_when_corrupt() {
    let directory = tempfile::tempdir().unwrap();
    let audio = directory.path().join("audio");
    std::fs::create_dir_all(&audio).unwrap();
    let bytes = b"original-audio-fixture";
    let key = format!("{:x}.wav", Sha256::digest(bytes));
    std::fs::write(audio.join(&key), bytes).unwrap();
    let mut package = BackupPackage {
        format: "cijian-profile".into(),
        version: 3,
        checksum: String::new(),
        payload: json!({
            "profile":"p",
            "tables":{},
            "audioIncluded":true,
            "audioBlobs":[],
            "credentialsIncluded":false
        }),
    };
    attach_audio(directory.path(), std::slice::from_ref(&key), &mut package).unwrap();
    assert_eq!(package.payload["audioBlobs"][0]["blobKey"], key);
    let decoded = decode_audio_blobs(&package).unwrap();
    assert_eq!(decoded[0].1, bytes);

    let restored = tempfile::tempdir().unwrap();
    write_audio_blobs(restored.path(), &decoded).unwrap();
    assert_eq!(
        std::fs::read(restored.path().join("audio").join(&key)).unwrap(),
        bytes
    );
    assert!(write_audio_blobs(restored.path(), &decoded).is_err());

    let mut corrupt = package.clone();
    corrupt.payload["audioBlobs"][0]["bytes"] = STANDARD.encode(b"tampered").into();
    assert!(decode_audio_blobs(&corrupt).is_err());

    let missing = tempfile::tempdir().unwrap();
    assert!(attach_audio(missing.path(), &[key], &mut package).is_err());
}
