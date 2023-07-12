use std::collections::VecDeque;
use std::fmt::Display;

use httptest::matchers::*;
use httptest::responders::*;
use httptest::{Expectation, Server};
use tempdir::TempDir;
use tokio::fs::{read_dir, read_to_string};

use crate::args::Args;

fn test_setup(url: &str) -> (Args, Server, TempDir) {
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

fn dump_tmp_contents(contents: &Vec<String>) {
    println!("Temp dir contents:");

    for f in contents {
        println!("  {}", f);
    }
}

async fn get_tmp_contents(tmpdir: &TempDir) -> Vec<String> {
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

                    if file_type.is_dir() {
                        contents.push(format!("{}/", rel_path.display()));
                        process_paths.push_back(full_path);
                    } else {
                        contents.push(format!("{}", rel_path.display()));
                    }
                }
            }
        }
    }

    contents
}

fn check_tmp_contents<S1, S2>(contents: &[S1], expected: &[S2])
where
    S1: ToString + Display,
    S2: ToString + Display,
{
    for s1 in contents {
        assert!(
            expected.iter().any(|s2| s2.to_string() == s1.to_string()),
            "Download directory contains file {s1}, which is not expected"
        );
    }

    for s2 in expected {
        assert!(
            expected.iter().any(|s1| s1.to_string() == s2.to_string()),
            "Download directory should contain file {s2}"
        );
    }
}

async fn check_tmp_file(tmpdir: &TempDir, file: &str, expected_content: &str) {
    let mut path = tmpdir.path().to_path_buf();
    path.push(file);

    let content = read_to_string(&path)
        .await
        .expect(&format!("Failed to read file {}", path.display()));

    assert_eq!(
        content,
        expected_content,
        "Contents of file {} incorrect",
        path.display()
    );
}

fn build_html_anchors_doc(anchors: &[String]) -> String {
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

#[tokio::test]
async fn test_404() {
    let (args, server, tmpdir) = test_setup("/");

    // Configure the server to expect a single GET /test request and respond with a 404 status code.
    server.expect(
        Expectation::matching(request::method_path("GET", "/")).respond_with(status_code(404)),
    );

    // Process
    let result = super::process(args).await;

    // Check results
    println!("{:?}", result);
    assert!(
        matches!(result, Err(e) if e.to_string().starts_with("Status 404 Not Found fetching http://"))
    );

    let dir_cnt = get_tmp_contents(&tmpdir).await;
    dump_tmp_contents(&dir_cnt);
    let cnt: [&str; 0] = [];
    check_tmp_contents(&dir_cnt, &cnt);
}

#[tokio::test]
async fn test_single_file() {
    let (args, server, tmpdir) = test_setup("/file");

    let file_content = "Hello, world!";

    // Configure the server to expect a single GET /file request and respond with the file content.
    server.expect(
        Expectation::matching(request::method_path("GET", "/file"))
            .respond_with(status_code(200).body(file_content)),
    );

    // Process
    let result = super::process(args).await;

    // Check results
    println!("{:?}", result);
    assert!(matches!(result, Ok(())));

    let dir_cnt = get_tmp_contents(&tmpdir).await;
    dump_tmp_contents(&dir_cnt);
    check_tmp_contents(
        &dir_cnt,
        &["download/", "download/.etags.json", "download/__file.dat"],
    );
    check_tmp_file(&tmpdir, "download/__file.dat", file_content).await;
}

#[tokio::test]
async fn test_single_html_empty() {
    let (args, server, tmpdir) = test_setup("/");

    // Build document with no anchors
    let html_doc = build_html_anchors_doc(&[]);

    // Configure the server to expect a single GET / request and respond with the html document.
    server.expect(
        Expectation::matching(request::method_path("GET", "/")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(html_doc),
        ),
    );

    // Process
    let result = super::process(args).await;

    // Check results
    println!("{:?}", result);
    assert!(matches!(result, Ok(())));

    let dir_cnt = get_tmp_contents(&tmpdir).await;
    dump_tmp_contents(&dir_cnt);
    let cnt: [&str; 0] = [];
    check_tmp_contents(&dir_cnt, &cnt);
}

#[tokio::test]
async fn test_single_html() {
    let (args, server, tmpdir) = test_setup("/root");

    // Build document with some anchors
    let html_doc = build_html_anchors_doc(&[
        "../notrelative".to_string(),
        "file://some_file".to_string(),
        "http://example.com".to_string(),
        "#".to_string(),
        "#hash".to_string(),
        "?".to_string(),
        "?query".to_string(),
        "?query#hash".to_string(),
        server.url("/notrelative").to_string(),
        server.url("/root/file1").to_string(), // Valid full URL
        "root/file2".to_string(),              // Valid relative URL
    ]);

    let file_content = "Hello, world!";

    // Configure the server to expect a single GET /root request and respond with the html document
    server.expect(
        Expectation::matching(request::method_path("GET", "/root")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(html_doc),
        ),
    );

    // Configure the server to expect a single GET /root/file1 request and respond with the file content.
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/file1"))
            .respond_with(status_code(200).body(file_content)),
    );

    // Configure the server to expect a single GET /root/file2 request and respond with the file content.
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/file2"))
            .respond_with(status_code(200).body(file_content)),
    );

    // Process
    let result = super::process(args).await;

    // Check results
    println!("{:?}", result);
    assert!(matches!(result, Ok(())));

    let dir_cnt = get_tmp_contents(&tmpdir).await;
    dump_tmp_contents(&dir_cnt);
    check_tmp_contents(
        &dir_cnt,
        &[
            "download/.etags.json",
            "download/",
            "download/file1",
            "download/file2",
        ],
    );
    check_tmp_file(&tmpdir, "download/file1", file_content).await;
    check_tmp_file(&tmpdir, "download/file2", file_content).await;
}

#[tokio::test]
async fn test_multi_html() {
    let (args, server, tmpdir) = test_setup("/root/");

    // Build main document with some anchors
    let anchors = (1..=10).map(|s| format!("{}/", s)).collect::<Vec<_>>();

    let main_html_doc = build_html_anchors_doc(&anchors);

    let file_content = "Hello, world!";

    // Configure the server to expect a single GET /root request and respond with the html document
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(main_html_doc),
        ),
    );

    // Configure the server to serve sub pages
    let sub_anchors = (1..=10).map(|s| format!("{}", s)).collect::<Vec<_>>();

    for page in 1..=10 {
        let html_doc = build_html_anchors_doc(&sub_anchors);

        server.expect(
            Expectation::matching(request::method_path("GET", format!("/root/{}/", page)))
                .respond_with(
                    status_code(200)
                        .append_header("Content-Type", "text/html")
                        .body(html_doc),
                ),
        );

        // Serve up the file content
        for a in sub_anchors.iter() {
            server.expect(
                Expectation::matching(request::method_path("GET", format!("/root/{page}/{a}")))
                    .respond_with(status_code(200).body(file_content)),
            );
        }
    }

    // Process
    let result = super::process(args).await;

    // Check results
    println!("{:?}", result);
    assert!(matches!(result, Ok(())));

    let mut expected_contents = vec!["download/.etags.json".to_string(), "download/".to_string()];
    for i in 1..=10 {
        expected_contents.push(format!("download/{i}/"));

        for j in 1..=10 {
            expected_contents.push(format!("download/{i}/{j}"));
        }
    }

    let dir_cnt = get_tmp_contents(&tmpdir).await;
    dump_tmp_contents(&dir_cnt);
    check_tmp_contents(&dir_cnt, &expected_contents);

    for i in 1..=10 {
        for j in 1..=10 {
            check_tmp_file(&tmpdir, &format!("download/{i}/{j}"), file_content).await;
        }
    }
}
