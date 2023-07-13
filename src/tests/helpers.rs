// Helper functions

use std::collections::VecDeque;
use std::fmt::Display;
use std::ops::Deref;
use std::path::PathBuf;

use httptest::Server;
use tempdir::TempDir;
use tokio::fs::{read_dir, read_to_string, File};
use tokio::io::AsyncWriteExt;

use crate::args::Args;
use crate::etags::ETags;

pub fn test_setup(url: &str) -> (Args, Server, TempDir) {
    let server = Server::run();

    let url = server.url(url);

    let tmpdir = TempDir::new("mirrorurl_test").expect("Failed to create tmp dir");

    let mut path = tmpdir.path().to_path_buf();
    path.push("download");

    let args = Args {
        url: url.to_string(),
        target: path.to_string_lossy().to_string(),
        debug: 1,
        ..Args::default()
    };

    (args, server, tmpdir)
}

pub fn build_html_anchors_doc<A>(anchors: &[A]) -> String
where
    A: Display,
{
    let mut doc = String::new();

    doc.push_str(
        r#"<DOCTYPE html>
<html>
    <head>
    </head>
    <body>"#,
    );

    for a in anchors {
        doc.push_str(&format!("        <a href=\"{a}\">Anchor: {a}</a>\n"));
    }

    doc.push_str(
        "\
    </body>
</html>",
    );

    doc
}

pub fn generate_etags_json(etag_values: Vec<(String, String)>) -> String {
    let mut etags = ETags::default();

    for (url, etag) in etag_values.into_iter() {
        etags.add(url, etag);
    }

    let mut bytes = Vec::new();

    etags.write(&mut bytes).expect("Failed to serialise etags");

    String::from_utf8(bytes).expect("Failed to convert serialised etags to string")
}

pub async fn generate_skiplist_json(tmpdir: &TempDir, values: Vec<&str>) -> (PathBuf, String) {
    let mut path = PathBuf::from(tmpdir.path());
    path.push("skiplist.json");

    let json = serde_json::to_string(&values).expect("Failed to serialise array");

    let mut fh = File::create(&path).await.expect("Error creating skip list");
    fh.write_all(json.as_bytes())
        .await
        .expect("Error writing skip list");

    (path, json)
}

pub async fn check_tmp_contents<S1, S2>(tmpdir: &TempDir, expected: &[TmpFile<S1, S2>])
where
    S1: Deref<Target = str> + Display,
    S2: Deref<Target = str> + Display,
{
    let contents = get_tmp_contents(tmpdir).await;

    dump_tmp_contents(&contents);

    // Check the correct files exist
    compare_tmp_contents(&contents, expected, false);
    compare_tmp_contents(expected, &contents, true);
}

pub enum TmpFile<S1, S2> {
    Dir(S1),
    File(S1, S2),
}

async fn get_tmp_contents(tmpdir: &TempDir) -> Vec<TmpFile<String, String>> {
    let mut contents = Vec::new();

    let mut process_paths = VecDeque::new();
    process_paths.push_back(tmpdir.path().to_path_buf());

    while let Some(d) = process_paths.pop_front() {
        let mut paths = read_dir(&d)
            .await
            .expect(&format!("Failed to read directory {}", d.display()));

        loop {
            let dirent = paths.next_entry().await.expect(&format!(
                "Failed to read next directory entry {}",
                d.display()
            ));
            match dirent {
                None => break,
                Some(dirent) => {
                    let full_path = dirent.path();

                    let rel_path = full_path
                        .strip_prefix(tmpdir.path())
                        .expect("Failed to remove tmpdir prefix");

                    let file_type = dirent.file_type().await.expect(&format!(
                        "Error getting file type for {}",
                        rel_path.display()
                    ));

                    let rel_name = rel_path
                        .to_str()
                        .expect("File name could not be converted to string")
                        .to_string();

                    if file_type.is_dir() {
                        process_paths.push_back(full_path);

                        contents.push(TmpFile::Dir(rel_name));
                    } else {
                        let content = read_to_string(&full_path)
                            .await
                            .expect(&format!("Failed to read file {}", full_path.display()));

                        contents.push(TmpFile::File(rel_name, content));
                    }
                }
            }
        }
    }

    contents
}

fn dump_tmp_contents(contents: &[TmpFile<String, String>]) {
    println!("Temp dir contents:");

    for f in contents {
        match f {
            TmpFile::Dir(d) => println!("  {d}/"),
            TmpFile::File(f, c) => println!("  {f} ({} bytes)", c.len()),
        }
    }
}

fn compare_tmp_contents<S1, S2, S3, S4>(
    c1: &[TmpFile<S1, S2>],
    c2: &[TmpFile<S3, S4>],
    compare: bool,
) where
    S1: Deref<Target = str> + Display,
    S2: Deref<Target = str> + Display,
    S3: Deref<Target = str> + Display,
    S4: Deref<Target = str> + Display,
{
    // Check the correct files exist
    for f1 in c1 {
        match f1 {
            TmpFile::Dir(d1) => {
                assert!(
                    c2.iter()
                        .filter_map(|d2| match d2 {
                            TmpFile::Dir(d) => Some(d),
                            _ => None,
                        })
                        .any(|d2| { d1.deref() == d2.deref() }),
                    "Download directory contains file {d1}, which is not expected"
                );
            }
            TmpFile::File(f1, cnt1) => {
                match c2
                    .iter()
                    .find(|f| matches!(f, TmpFile::File(f2, _) if f1.deref() == f2.deref()))
                {
                    Some(TmpFile::File(f2, cnt2)) => {
                        if compare {
                            assert_eq!(
                                cnt1.deref(),
                                cnt2.deref(),
                                "Contents of file {f2} incorrect"
                            );
                        }
                    }
                    _ => {
                        panic!("Download directory contains file {f1}, which is not expected");
                    }
                }
            }
        }
    }
}
