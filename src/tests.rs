use std::collections::VecDeque;
use std::fmt::Display;
use std::ops::Deref;

use httptest::matchers::*;
use httptest::responders::*;
use httptest::{Expectation, Server};
use tempdir::TempDir;
use tokio::fs::{read_dir, read_to_string};

use crate::args::Args;
use crate::etags::ETags;

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

    check_tmp_contents(&tmpdir, &[] as &[TmpFile<&str, &str>; 0]).await;
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

    check_tmp_contents(
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/.etags.json", "{}"),
            TmpFile::File("download/__file.dat", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_single_file_etag() {
    let (args, server, tmpdir) = test_setup("/file");

    let file_content = "Hello, world!";

    let etag_value = "etagvalue";

    let etags_content = generate_etags_json(vec![(
        server.url("/file").to_string(),
        etag_value.to_string(),
    )]);

    // Configure the server to expect a single GET /file request and respond with the file content and etag
    server.expect(
        Expectation::matching(all_of!(
            request::method_path("GET", "/file"),
            request::headers(not(contains(key("if-none-match")))),
        ))
        .respond_with(
            status_code(200)
                .append_header("ETag", "etagvalue")
                .body(file_content),
        ),
    );

    // Configure the server to expect a single GET /file request with a valid If-None-Matches header and respond with 304 not modified
    server.expect(
        Expectation::matching(all_of!(
            request::method_path("GET", "/file"),
            request::headers(contains(("if-none-match", etag_value.clone()))),
        ))
        .respond_with(status_code(304)),
    );

    // First process
    let result = super::process(args.clone()).await;

    // Check results
    println!("{:?}", result);
    assert!(matches!(result, Ok(())));

    check_tmp_contents(
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/.etags.json", etags_content.as_str()),
            TmpFile::File("download/__file.dat", file_content),
        ],
    )
    .await;

    // Second process
    let result = super::process(args).await;

    // Check results
    println!("{:?}", result);
    assert!(matches!(result, Ok(())));

    check_tmp_contents(
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/.etags.json", etags_content.as_str()),
            TmpFile::File("download/__file.dat", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_single_file_no_etag() {
    let (mut args, server, tmpdir) = test_setup("/file");

    args.no_etags = true;

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

    check_tmp_contents(
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/__file.dat", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_single_html_empty() {
    let (args, server, tmpdir) = test_setup("/");

    // Build document with no anchors
    let html_doc = build_html_anchors_doc(&[] as &[&str; 0]);

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

    check_tmp_contents(&tmpdir, &[] as &[TmpFile<&str, &str>; 0]).await;
}

#[tokio::test]
async fn test_single_html() {
    let (args, server, tmpdir) = test_setup("/root");

    // Build document with some anchors
    let html_doc = build_html_anchors_doc(&[
        "../notrelative",
        "file://some_file",
        "http://example.com",
        "#",
        "#hash",
        "?",
        "?query",
        "?query#hash",
        &server.url("/notrelative").to_string(),
        &server.url("/root/file1").to_string(), // Valid full URL
        "root/file2",                           // Valid relative URL
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

    check_tmp_contents(
        &tmpdir,
        &[
            TmpFile::File("download/.etags.json", "{}"),
            TmpFile::Dir("download"),
            TmpFile::File("download/file1", file_content),
            TmpFile::File("download/file2", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_single_xhtml() {
    let (args, server, tmpdir) = test_setup("/root");

    // Build document with some anchors
    let html_doc = build_html_anchors_doc(&[&server.url("/root/file1").to_string()]);

    let file_content = "Hello, world!";

    // Configure the server to expect a single GET /root request and respond with the html document
    server.expect(
        Expectation::matching(request::method_path("GET", "/root")).respond_with(
            status_code(200)
                .append_header("Content-Type", "application/xhtml+xml")
                .body(html_doc),
        ),
    );

    // Configure the server to expect a single GET /root/file1 request and respond with the file content.
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/file1"))
            .respond_with(status_code(200).body(file_content)),
    );

    // Process
    let result = super::process(args).await;

    // Check results
    println!("{:?}", result);
    assert!(matches!(result, Ok(())));

    check_tmp_contents(
        &tmpdir,
        &[
            TmpFile::File("download/.etags.json", "{}"),
            TmpFile::Dir("download"),
            TmpFile::File("download/file1", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_single_html_duplicate() {
    let (args, server, tmpdir) = test_setup("/root");

    // Build document with some anchors
    let html_doc =
        build_html_anchors_doc(&["root/file1", server.url("/root/file1").to_string().as_str()]);

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

    // Process
    let result = super::process(args).await;

    // Check results
    println!("{:?}", result);
    assert!(matches!(result, Ok(())));

    check_tmp_contents(
        &tmpdir,
        &[
            TmpFile::File("download/.etags.json", "{}"),
            TmpFile::Dir("download"),
            TmpFile::File("download/file1", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_multi_html() {
    let (args, server, tmpdir) = test_setup("/root/");

    // File content
    let file_content = "Hello, world!";

    // Build main document with some anchors
    let sub_pages = (1..=16).collect::<Vec<_>>();

    let main_anchors = sub_pages
        .iter()
        .map(|s| format!("{}/", s))
        .collect::<Vec<_>>();

    let main_html_doc = build_html_anchors_doc(&main_anchors);

    // Configure the server to expect a single GET /root request and respond with the main html document
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(main_html_doc),
        ),
    );

    // Configure the server to serve sub pages
    let html_doc = build_html_anchors_doc(&sub_pages);

    for page in sub_pages.iter() {
        server.expect(
            Expectation::matching(request::method_path("GET", format!("/root/{}/", page)))
                .respond_with(
                    status_code(200)
                        .append_header("Content-Type", "text/html")
                        .body(html_doc.clone()),
                ),
        );

        // Serve up the file content
        for a in sub_pages.iter() {
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

    let mut expected_contents = vec![
        TmpFile::File("download/.etags.json".to_string(), "{}"),
        TmpFile::Dir("download".to_string()),
    ];

    for i in sub_pages.iter() {
        expected_contents.push(TmpFile::Dir(format!("download/{i}")));

        for j in sub_pages.iter() {
            expected_contents.push(TmpFile::File(format!("download/{i}/{j}"), file_content));
        }
    }

    check_tmp_contents(&tmpdir, &expected_contents).await;
}

// Helper functions

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

fn dump_tmp_contents(contents: &[TmpFile<String, String>]) {
    println!("Temp dir contents:");

    for f in contents {
        match f {
            TmpFile::Dir(d) => println!("  {d}/"),
            TmpFile::File(f, c) => println!("  {f} ({} bytes)", c.len()),
        }
    }
}

enum TmpFile<S1, S2> {
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

async fn check_tmp_contents<S1, S2>(tmpdir: &TempDir, expected: &[TmpFile<S1, S2>])
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

fn build_html_anchors_doc<A>(anchors: &[A]) -> String
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

fn generate_etags_json(etag_values: Vec<(String, String)>) -> String {
    let mut etags = ETags::default();

    for (url, etag) in etag_values.into_iter() {
        etags.add(url, etag);
    }

    let mut bytes = Vec::new();

    etags.write(&mut bytes).expect("Failed to serialise etags");

    String::from_utf8(bytes).expect("Failed to convert serialised etags to string")
}
