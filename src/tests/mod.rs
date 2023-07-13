use httptest::matchers::*;
use httptest::responders::*;
use httptest::Expectation;

mod helpers;
use helpers::*;

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

#[tokio::test]
async fn test_multi_html_skiplist() {
    let (mut args, server, tmpdir) = test_setup("/root/");

    // Generate skip list
    let (skip_path, skip_content) = generate_skiplist_json(&tmpdir, vec!["1", "2/", "3/1"]).await;
    args.skip_file = Some(skip_path.to_str().unwrap().to_string());

    // File content
    let file_content = "Hello, world!";

    // Build main document with some anchors
    let sub_pages = (1..=4).collect::<Vec<_>>();

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
        TmpFile::File("skiplist.json".to_string(), skip_content.as_str()),
        TmpFile::File("download/.etags.json".to_string(), "{}"),
        TmpFile::Dir("download".to_string()),
    ];

    for i in sub_pages.iter() {
        if *i > 2 {
            expected_contents.push(TmpFile::Dir(format!("download/{i}")));

            for j in sub_pages.iter() {
                if *i != 3 || *j != 1 {
                    expected_contents
                        .push(TmpFile::File(format!("download/{i}/{j}"), file_content));
                }
            }
        }
    }

    check_tmp_contents(&tmpdir, &expected_contents).await;
}

#[tokio::test]
async fn test_redirect() {
    let (args, server, tmpdir) = test_setup("/root");

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
                .body(html_doc),
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
            TmpFile::File("download/afterfile", file_content),
        ],
    )
    .await;
}
