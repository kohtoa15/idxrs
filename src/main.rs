use idxrs::IdxValue;
use idxrs::IdxCursor;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::str::FromStr;
use std::path::Path;
use std::path::PathBuf;

fn generate_batch_path(base_dir: &str, digit: u8, batch: usize) -> PathBuf {
    let mut buf = Box::new(Path::new(base_dir)).to_path_buf();
    buf.push(format!("digit-{}-batch-{}", digit, batch));
    return buf;
}

fn main() {
    let mut args = std::env::args().skip(1);
    // Get cmd line args
    let path = args.next().unwrap();
    let base_dir = args.next().unwrap();
    let batch_num = usize::from_str(args.next().unwrap().trim()).unwrap();
    // Open specified idx file
    let file = File::open(path).unwrap();
    let mut cursor = IdxCursor::new(BufReader::new(file)).unwrap();
    let dimensions = cursor.dimensions.clone();

    #[cfg(debug_assertions)]
    println!("dimension: {}", &dimensions[0]);

    std::fs::create_dir_all(base_dir.as_str()).unwrap();
    
    let mut batch_counts: [usize; 10] = [0; 10];

    // display img labels
    for i in 0..dimensions[0] {
        if let IdxValue::UnsignedByte(b) = cursor.get(&vec![i]).unwrap() {
            if b < 10 {
                // getting batch count for digit
                let cnt = batch_counts[b as usize].clone();
                // increase count for digit
                if cnt < batch_num {
                    batch_counts[b as usize] += 1;
                } else {
                    batch_counts[b as usize] = 0;
                }
                // write index to digit batch files
                let mut buf = generate_batch_path(base_dir.as_str(), b, cnt);
                std::fs::create_dir_all(buf.as_path()).unwrap();
                buf.push("input.txt");
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .append(true)
                    .open(buf.as_path())
                    .unwrap();
                writeln!(file, "{}", i).unwrap();
            }
        }
    }
}
