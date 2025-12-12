use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::thread::sleep;
use std::time::Duration;

fn tail_file<F>(path: &str, interval: Duration, mut callback: F) -> std::io::Result<()>
where
    F: FnMut(&str),
{
    let mut file = File::open(path)?;
    let mut pos = file.seek(SeekFrom::End(0))?;

    loop {
        sleep(interval);

        file = File::open(path)?;
        let metadata = file.metadata()?;

        if metadata.len() < pos {
            pos = 0;
        }

        file.seek(SeekFrom::Start(pos))?;
        let reader = BufReader::new(&file);

        for line in reader.lines() {
            let line = line?;
            callback(&line);
        }

        pos = file.stream_position()?;
    }
}
