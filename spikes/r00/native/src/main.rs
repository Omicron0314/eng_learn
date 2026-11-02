use rusqlite::{Connection, Result};

fn probe() -> Result<()> {
    let directory = tempfile::tempdir().expect("temporary probe directory");
    let path = directory.path().join("probe.sqlite");
    let mut db = Connection::open(&path)?;
    db.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;
        CREATE TABLE cards(id TEXT PRIMARY KEY, revision INTEGER NOT NULL);
        CREATE TABLE events(id TEXT PRIMARY KEY, card_id TEXT NOT NULL REFERENCES cards(id));
        CREATE TABLE receipts(id TEXT PRIMARY KEY, result TEXT NOT NULL);",
    )?;
    let tx = db.transaction()?;
    tx.execute("INSERT INTO cards VALUES ('original', 1)", [])?;
    tx.execute("INSERT INTO events VALUES ('event', 'original')", [])?;
    tx.execute("INSERT INTO receipts VALUES ('operation', 'saved')", [])?;
    tx.commit()?;
    {
        let tx = db.transaction()?;
        tx.execute(
            "UPDATE cards SET revision=2 WHERE id='original' AND revision=1",
            [],
        )?;
        assert!(
            tx.execute("INSERT INTO events VALUES ('event', 'original')", [])
                .is_err()
        );
        // Drop rolls the entire failed command back.
    }
    drop(db);
    let first = Connection::open(&path)?;
    let second = Connection::open(&path)?;
    assert_eq!(
        first.query_row("SELECT revision FROM cards", [], |r| r.get::<_, i64>(0))?,
        1
    );
    assert_eq!(
        first.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))?,
        1
    );
    assert_eq!(
        first.query_row("SELECT result FROM receipts", [], |r| r.get::<_, String>(0))?,
        "saved"
    );
    assert_eq!(
        first.execute(
            "UPDATE cards SET revision=2 WHERE id='original' AND revision=1",
            []
        )?,
        1
    );
    assert_eq!(
        second.execute(
            "UPDATE cards SET revision=2 WHERE id='original' AND revision=1",
            []
        )?,
        0
    );
    Ok(())
}

fn main() -> Result<()> {
    if std::env::args().any(|arg| arg == "--file-dialog") {
        let selected = rfd::FileDialog::new()
            .set_title("R00 探针：取消即可，不要选择私人文件")
            .pick_file();
        println!(
            "{}",
            serde_json::json!({"fileDialogReturned": true, "cancelled": selected.is_none()})
        );
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--audio") {
        let stream = rodio::OutputStreamBuilder::open_default_stream();
        match stream {
            Ok(stream) => {
                let sink = rodio::Sink::connect_new(stream.mixer());
                sink.set_volume(0.0);
                sink.append(rodio::buffer::SamplesBuffer::new(
                    1,
                    44100,
                    vec![0.0_f32; 88200],
                ));
                sink.pause();
                sink.set_speed(1.25);
                let seek = sink.try_seek(std::time::Duration::from_secs(1)).is_ok();
                println!(
                    "{}",
                    serde_json::json!({"audioDeviceOpened": true, "seek": seek, "rate": sink.speed(), "audiblePlaybackTested": false})
                );
            }
            Err(error) => println!(
                "{}",
                serde_json::json!({"audioDeviceOpened": false, "reason": error.to_string()})
            ),
        }
        return Ok(());
    }
    probe()?;
    println!(
        "{}",
        serde_json::json!({"sqlite": rusqlite::version(), "temporaryDatabase": true,
        "businessEventReceiptCommit": true, "eventFailureRollsBackBusiness": true,
        "reopenPreservesData": true, "secondConnectionStaleRevisionRejected": true})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn transaction_failure_reopen_and_competing_revision() {
        super::probe().unwrap();
    }
}
