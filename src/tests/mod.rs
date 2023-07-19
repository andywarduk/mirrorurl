use httptest::matchers::*;
use httptest::responders::*;
use httptest::Expectation;

mod helpers;
use helpers::*;

use super::async_main;
use crate::stats::Stats;

#[tokio::test]
async fn test_404() {
    let (args, mut server, tmpdir) = test_setup("/");

    // Configure the server to expect a single GET /test request and respond with a 404 status code.
    server.expect(
        Expectation::matching(request::method_path("GET", "/")).respond_with(status_code(404)),
    );

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_errored();

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/")),
        format!("ERROR: Status 404 Not Found fetching {}", server.url("/")),
        "INFO: 0 documents parsed (0 bytes)".to_string(),
        "INFO: 0 files downloaded (0 bytes), 0 not modified, 0 skipped, 1 errored".to_string(),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[] as &[TmpFile<&str, &str>; 0],
    )
    .await;
}

#[tokio::test]
async fn test_single_file() {
    let (args, mut server, tmpdir) = test_setup("/file");

    let file_content = "Hello, world!";

    // Configure the server to expect a single GET /file request and respond with the file content.
    server.expect(
        Expectation::matching(request::method_path("GET", "/file"))
            .respond_with(status_code(200).body(file_content)),
    );

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_download(file_content.len());

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/file")),
        format!(
            "INFO: Downloading {} to {}/download/__file.dat (size {})",
            server.url("/file"),
            tmpdir.path().display(),
            file_content.len()
        ),
        "INFO: 0 documents parsed (0 bytes)".to_string(),
        format!(
            "INFO: 1 file downloaded ({} bytes), 0 not modified, 0 skipped, 0 errored",
            file_content.len()
        ),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/__file.dat", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_single_file_etag() {
    let (args, mut server, tmpdir) = test_setup("/file");

    let file_content = "Hello, world!";

    let etag_value = "etagvalue";

    let etags_content = generate_etags_json(vec![(
        server.url("/file").to_string(),
        etag_value.to_string(),
    )]);

    // **** First process ****

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

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_download(file_content.len());

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/file")),
        format!(
            "INFO: Downloading {} to {}/download/__file.dat (size {})",
            server.url("/file"),
            tmpdir.path().display(),
            file_content.len()
        ),
        "INFO: 0 documents parsed (0 bytes)".to_string(),
        format!(
            "INFO: 1 file downloaded ({} bytes), 0 not modified, 0 skipped, 0 errored",
            file_content.len()
        ),
    ];

    // Process
    let result = async_main(args.clone()).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/.etags.json", etags_content.as_str()),
            TmpFile::File("download/__file.dat", file_content),
        ],
    )
    .await;

    // **** Second process ****

    // Configure the server to expect a single GET /file request with a valid If-None-Matches header and respond with 304 not modified
    server.expect(
        Expectation::matching(all_of!(
            request::method_path("GET", "/file"),
            request::headers(contains(("if-none-match", etag_value.clone()))),
        ))
        .respond_with(status_code(304)),
    );

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_not_modified();

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/file")),
        format!("INFO: {} is not modified", server.url("/file"),),
        "INFO: 0 documents parsed (0 bytes)".to_string(),
        "INFO: 0 files downloaded (0 bytes), 1 not modified, 0 skipped, 0 errored".to_string(),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
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
    let (mut args, mut server, tmpdir) = test_setup("/file");

    args.no_etags = true;

    let file_content = "Hello, world!";

    // Configure the server to expect a single GET /file request and respond with the file content.
    server.expect(
        Expectation::matching(request::method_path("GET", "/file"))
            .respond_with(status_code(200).body(file_content)),
    );

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_download(file_content.len());

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/file")),
        format!(
            "INFO: Downloading {} to {}/download/__file.dat (size {})",
            server.url("/file"),
            tmpdir.path().display(),
            file_content.len()
        ),
        "INFO: 0 documents parsed (0 bytes)".to_string(),
        format!(
            "INFO: 1 file downloaded ({} bytes), 0 not modified, 0 skipped, 0 errored",
            file_content.len()
        ),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
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
    let (args, mut server, tmpdir) = test_setup("/");

    // Build document with no anchors
    let html_doc = build_html_anchors_doc(&[] as &[&str; 0]);

    // Configure the server to expect a single GET / request and respond with the html document.
    server.expect(
        Expectation::matching(request::method_path("GET", "/")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(html_doc.clone()),
        ),
    );

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_html(html_doc.len());

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/")),
        format!("INFO: 1 document parsed ({} bytes)", html_doc.len()),
        "INFO: 0 files downloaded (0 bytes), 0 not modified, 0 skipped, 0 errored".to_string(),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[] as &[TmpFile<&str, &str>; 0],
    )
    .await;
}

#[tokio::test]
async fn test_single_html_404() {
    let (args, mut server, tmpdir) = test_setup("/");

    // Build document single anchor
    let html_doc = build_html_anchors_doc(&["file"]);

    // Configure the server to expect a single GET / request and respond with the html document.
    server.expect(
        Expectation::matching(request::method_path("GET", "/")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(html_doc.clone()),
        ),
    );

    // Configure the server to expect a single GET /file request and respond with 404.
    server.expect(
        Expectation::matching(request::method_path("GET", "/file")).respond_with(status_code(404)),
    );

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_html(html_doc.len());
    expected_stats.add_errored();

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/")),
        format!("INFO: Fetching {}", server.url("/file")),
        format!(
            "ERROR: Status 404 Not Found fetching {}",
            server.url("/file")
        ),
        format!("INFO: 1 document parsed ({} bytes)", html_doc.len()),
        "INFO: 0 files downloaded (0 bytes), 0 not modified, 0 skipped, 1 errored".to_string(),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[] as &[TmpFile<&str, &str>; 0],
    )
    .await;
}

#[tokio::test]
async fn test_single_html() {
    let (args, mut server, tmpdir) = test_setup("/root");

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
                .body(html_doc.clone()),
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

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_html(html_doc.len());

    for _ in 0..2 {
        expected_stats.add_download(file_content.len());
    }

    for _ in 0..9 {
        expected_stats.add_skipped();
    }

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/root")),
        format!("INFO: Fetching {}", server.url("/root/file1")),
        format!("INFO: Fetching {}", server.url("/root/file2")),
        format!(
            "INFO: Skipping {}: URL is not relative to the base URL",
            server.url("/notrelative")
        ),
        "INFO: Skipping file://some_file/: The transport is not supported".to_string(),
        "INFO: Skipping http://example.com/: URL is not relative to the base URL".to_string(),
        format!("INFO: Skipping {}#: URL is a fragment", server.url("/root")),
        format!(
            "INFO: Skipping {}#hash: URL is a fragment",
            server.url("/root")
        ),
        format!("INFO: Skipping {}: URL has a query", server.url("/root?")),
        format!(
            "INFO: Skipping {}: URL has a query",
            server.url("/root?query")
        ),
        format!(
            "INFO: Skipping {}#hash: URL is a fragment",
            server.url("/root?query")
        ),
        format!(
            "INFO: Downloading {} to {}/download/file1 (size {})",
            server.url("/root/file1"),
            tmpdir.path().display(),
            file_content.len()
        ),
        format!(
            "INFO: Downloading {} to {}/download/file2 (size {})",
            server.url("/root/file2"),
            tmpdir.path().display(),
            file_content.len()
        ),
        format!("INFO: 1 document parsed ({} bytes)", html_doc.len()),
        format!(
            "INFO: 2 files downloaded ({} bytes), 0 not modified, 9 skipped, 0 errored",
            file_content.len() * 2
        ),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/file1", file_content),
            TmpFile::File("download/file2", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_single_xhtml() {
    let (args, mut server, tmpdir) = test_setup("/root");

    // Build document with some anchors
    let html_doc = build_html_anchors_doc(&[&server.url("/root/file1").to_string()]);

    let file_content = "Hello, world!";

    // Configure the server to expect a single GET /root request and respond with the html document
    server.expect(
        Expectation::matching(request::method_path("GET", "/root")).respond_with(
            status_code(200)
                .append_header("Content-Type", "application/xhtml+xml")
                .body(html_doc.clone()),
        ),
    );

    // Configure the server to expect a single GET /root/file1 request and respond with the file content.
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/file1"))
            .respond_with(status_code(200).body(file_content)),
    );

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_html(html_doc.len());
    expected_stats.add_download(file_content.len());

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/root")),
        format!("INFO: Fetching {}", server.url("/root/file1")),
        format!(
            "INFO: Downloading {} to {}/download/file1 (size {})",
            server.url("/root/file1"),
            tmpdir.path().display(),
            file_content.len()
        ),
        format!("INFO: 1 document parsed ({} bytes)", html_doc.len()),
        format!(
            "INFO: 1 file downloaded ({} bytes), 0 not modified, 0 skipped, 0 errored",
            file_content.len()
        ),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/file1", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_single_html_duplicate() {
    let (args, mut server, tmpdir) = test_setup("/root");

    // Build document with some anchors
    let html_doc =
        build_html_anchors_doc(&["root/file1", server.url("/root/file1").to_string().as_str()]);

    let file_content = "Hello, world!";

    // Configure the server to expect a single GET /root request and respond with the html document
    server.expect(
        Expectation::matching(request::method_path("GET", "/root")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(html_doc.clone()),
        ),
    );

    // Configure the server to expect a single GET /root/file1 request and respond with the file content.
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/file1"))
            .respond_with(status_code(200).body(file_content)),
    );

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_html(html_doc.len());
    expected_stats.add_download(file_content.len());

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/root")),
        format!("INFO: Fetching {}", server.url("/root/file1")),
        format!(
            "INFO: Downloading {} to {}/download/file1 (size {})",
            server.url("/root/file1"),
            tmpdir.path().display(),
            file_content.len()
        ),
        format!("INFO: 1 document parsed ({} bytes)", html_doc.len()),
        format!(
            "INFO: 1 file downloaded ({} bytes), 0 not modified, 0 skipped, 0 errored",
            file_content.len()
        ),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/file1", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_multi_html() {
    let (mut args, mut server, tmpdir) = test_setup("/root/");
    args.debug = 0;

    const SUB_PAGES: usize = 16;

    // Start expected stats
    let mut expected_stats = Stats::default();

    // Start expected messages
    let mut expected_messages = Vec::new();

    // Start expected contents
    let mut expected_contents = vec![TmpFile::Dir("download".to_string())];

    // File content
    let file_content = "Hello, world!";

    // Build main document with some anchors
    let main_anchors = (0..SUB_PAGES)
        .map(|s| format!("{}/", s))
        .collect::<Vec<_>>();

    let main_html_doc = build_html_anchors_doc(&main_anchors);

    // Configure the server to expect a single GET /root request and respond with the main html document
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(main_html_doc.clone()),
        ),
    );

    expected_stats.add_html(main_html_doc.len());
    expected_messages.push(format!("INFO: Fetching {}", server.url("/root/")));

    // Configure the server to serve sub pages
    let html_doc = build_html_anchors_doc(&(0..SUB_PAGES).collect::<Vec<_>>());

    for page in 0..SUB_PAGES {
        server.expect(
            Expectation::matching(request::method_path("GET", format!("/root/{}/", page)))
                .respond_with(
                    status_code(200)
                        .append_header("Content-Type", "text/html")
                        .body(html_doc.clone()),
                ),
        );

        expected_stats.add_html(html_doc.len());
        expected_messages.push(format!("INFO: Fetching {}/{page}/", server.url("/root")));

        // Serve up the file content
        for a in 0..SUB_PAGES {
            server.expect(
                Expectation::matching(request::method_path("GET", format!("/root/{page}/{a}")))
                    .respond_with(status_code(200).body(file_content)),
            );

            expected_contents.push(TmpFile::Dir(format!("download/{a}")));
            expected_messages.push(format!("INFO: Fetching {}/{page}/{a}", server.url("/root")));

            expected_stats.add_download(file_content.len());
            expected_contents.push(TmpFile::File(format!("download/{page}/{a}"), file_content));
            expected_messages.push(format!("INFO: Fetching {}/{page}/{a}", server.url("/root")));
            expected_messages.push(format!(
                "INFO: Downloading {}/{page}/{a} to {}/download/{page}/{a} (size {})",
                server.url("/root"),
                tmpdir.path().display(),
                file_content.len()
            ));
        }
    }

    expected_messages.push(format!(
        "INFO: {} documents parsed ({} bytes)",
        SUB_PAGES + 1,
        main_html_doc.len() + (SUB_PAGES * html_doc.len())
    ));
    expected_messages.push(format!(
        "INFO: {} files downloaded ({} bytes), 0 not modified, 0 skipped, 0 errored",
        SUB_PAGES * SUB_PAGES,
        SUB_PAGES * SUB_PAGES * file_content.len()
    ));

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &expected_contents,
    )
    .await;
}

#[tokio::test]
async fn test_multi_html_skiplist() {
    let (mut args, mut server, tmpdir) = test_setup("/root/");

    const SUB_PAGES: usize = 4;

    // Generate skip list
    let (skip_path, skip_content) = generate_skiplist_json(&tmpdir, vec!["1", "2/", "3/1"]).await;
    args.skip_file = Some(skip_path.to_str().unwrap().to_string());

    // Start expected stats
    let mut expected_stats = Stats::default();

    // Start expected messages
    let mut expected_messages = Vec::new();

    // Start expected contents
    let mut expected_contents = vec![
        TmpFile::File("skiplist.json".to_string(), skip_content.as_str()),
        TmpFile::Dir("download".to_string()),
    ];

    // File content
    let file_content = "Hello, world!";

    // Build main document with some anchors
    let main_anchors = (1..=SUB_PAGES)
        .map(|s| format!("{}/", s))
        .collect::<Vec<_>>();

    let main_html_doc = build_html_anchors_doc(&main_anchors);

    // Configure the server to expect a single GET /root request and respond with the main html document
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(main_html_doc.clone()),
        ),
    );

    expected_stats.add_html(main_html_doc.len());
    expected_messages.push(format!("INFO: Fetching {}", server.url("/root/")));

    // Configure the server to serve sub pages
    let html_doc = build_html_anchors_doc(&(1..=SUB_PAGES).collect::<Vec<_>>());

    for page in 1..=SUB_PAGES {
        if page > 2 {
            server.expect(
                Expectation::matching(request::method_path("GET", format!("/root/{}/", page)))
                    .respond_with(
                        status_code(200)
                            .append_header("Content-Type", "text/html")
                            .body(html_doc.clone()),
                    ),
            );

            expected_stats.add_html(html_doc.len());
            expected_messages.push(format!("INFO: Fetching {}/{page}/", server.url("/root")));

            // Serve up the file content
            for a in 1..=SUB_PAGES {
                if page != 3 || a != 1 {
                    server.expect(
                        Expectation::matching(request::method_path(
                            "GET",
                            format!("/root/{page}/{a}"),
                        ))
                        .respond_with(status_code(200).body(file_content)),
                    );

                    expected_contents.push(TmpFile::Dir(format!("download/{page}")));
                    expected_messages
                        .push(format!("INFO: Fetching {}/{page}/{a}", server.url("/root")));

                    expected_stats.add_download(file_content.len());
                    expected_contents
                        .push(TmpFile::File(format!("download/{page}/{a}"), file_content));
                    expected_messages
                        .push(format!("INFO: Fetching {}/{page}/{a}", server.url("/root")));
                    expected_messages.push(format!(
                        "INFO: Downloading {}/{page}/{a} to {}/download/{page}/{a} (size {})",
                        server.url("/root"),
                        tmpdir.path().display(),
                        file_content.len()
                    ));
                } else {
                    expected_stats.add_skipped();
                    expected_messages.push(format!(
                        "INFO: Skipping {}/{page}/{a}: Path is in the skip list",
                        server.url("/root")
                    ));
                }
            }
        } else {
            expected_stats.add_skipped();
            expected_messages.push(format!(
                "INFO: Skipping {}/{page}/: Path is in the skip list",
                server.url("/root")
            ));
        }
    }

    expected_messages.push(format!("INFO: 3 documents parsed (626 bytes)"));
    expected_messages.push(format!(
        "INFO: 7 files downloaded (91 bytes), 0 not modified, 3 skipped, 0 errored"
    ));

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &expected_contents,
    )
    .await;
}

#[tokio::test]
async fn test_redirect() {
    let (args, mut server, tmpdir) = test_setup("/root");

    // Build document with some anchors
    let html_doc = build_html_anchors_doc(&["beforefile", "extfile"]);

    let file_content = "Hello, world!";

    // Configure the server to expect a single GET /root request and respond with a redirect
    server.expect(
        Expectation::matching(request::method_path("GET", "/root"))
            .respond_with(status_code(301).append_header("Location", "/root/")),
    );

    // Configure the server to expect a single GET /root/ request and respond with the html document
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/")).respond_with(
            status_code(200)
                .append_header("Content-Type", "text/html")
                .body(html_doc.clone()),
        ),
    );

    // Configure the server to expect a single GET /root/beforefile request and respond with a relative redirect
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/beforefile"))
            .respond_with(status_code(301).append_header("Location", "/root/afterfile")),
    );

    // Configure the server to expect a single GET /root/beforefile request and respond with a relative redirect
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/afterfile"))
            .respond_with(status_code(200).body(file_content)),
    );

    // Configure the server to expect a single GET /root/extfile request and respond with an non-relative redirect
    server.expect(
        Expectation::matching(request::method_path("GET", "/root/extfile"))
            .respond_with(status_code(301).append_header("Location", "/other/extfile")),
    );

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_html(html_doc.len());
    expected_stats.add_download(file_content.len());
    expected_stats.add_skipped();

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/root")),
        format!("INFO: Fetching {}", server.url("/root/beforefile")),
        format!("INFO: Fetching {}", server.url("/root/extfile")),
        format!(
            "INFO: Skipping {}: Redirect to {} is not relative to the base URL",
            server.url("/root/extfile"),
            server.url("/other/extfile")
        ),
        format!(
            "INFO: Downloading {} to {}/download/afterfile (size {})",
            server.url("/root/afterfile"),
            tmpdir.path().display(),
            file_content.len()
        ),
        format!("INFO: 1 document parsed ({} bytes)", html_doc.len()),
        format!(
            "INFO: 1 file downloaded ({} bytes), 0 not modified, 1 skipped, 0 errored",
            file_content.len()
        ),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[
            TmpFile::Dir("download"),
            TmpFile::File("download/afterfile", file_content),
        ],
    )
    .await;
}

#[tokio::test]
async fn test_too_many_redirects() {
    let (args, mut server, tmpdir) = test_setup("/root");

    // Configure the server to expect a single GET /root request and respond with a redirect
    server.expect(
        Expectation::matching(request::method_path("GET", "/root"))
            .respond_with(status_code(301).append_header("Location", "/root/1")),
    );

    for i in 1..=10 {
        server.expect(
            Expectation::matching(request::method_path("GET", format!("/root/{}", i)))
                .respond_with(
                    status_code(301).append_header("Location", format!("/root/{}", i + 1)),
                ),
        );
    }

    // Build expected stats
    let mut expected_stats = Stats::default();
    expected_stats.add_skipped();

    // Build expected messages
    let expected_messages = [
        format!("INFO: Fetching {}", server.url("/root")),
        format!("INFO: Skipping {}: Too many redirects", server.url("/root")),
        "INFO: 0 documents parsed (0 bytes)".to_string(),
        "INFO: 0 files downloaded (0 bytes), 0 not modified, 1 skipped, 0 errored".to_string(),
    ];

    // Process
    let result = async_main(args).await;

    // Check results
    check_results(
        result,
        Ok(expected_stats),
        &expected_messages,
        &mut server,
        &tmpdir,
        &[] as &[TmpFile<&str, &str>; 0],
    )
    .await;
}
