use super::*;
#[test]
fn exact_request_has_only_question_and_single_image() {
    let config = Configuration::default();
    let body = request("synthetic question", &[1, 2, 3], &config);
    assert_eq!(body["temperature"], 0.4);
    assert_eq!(body["reasoning_effort"], "none");
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    let content = body["messages"][1]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["text"], "synthetic question");
    assert_eq!(
        content[1]["image_url"]["url"],
        "data:image/jpeg;base64,AQID"
    );
    assert!(body.get("max_tokens").is_none());
    assert!(body.get("max_completion_tokens").is_none());
    assert!(!config.enabled);
}
#[test]
fn stream_survives_every_utf8_fragment_boundary_and_retains_partial_on_disconnect() {
    let wire = "data: {\"choices\":[{\"delta\":{\"content\":\"å🦀\"}}]}\r\n\r\ndata: [DONE]\n\n";
    for split in 0..=wire.len() {
        let mut events = Events::default();
        let mut answer = String::new();
        events
            .feed(&wire.as_bytes()[..split], |s| answer.push_str(&s))
            .unwrap();
        events
            .feed(&wire.as_bytes()[split..], |s| answer.push_str(&s))
            .unwrap();
        events.finish().unwrap();
        assert_eq!(answer, "å🦀");
    }
    let mut events = Events::default();
    let mut answer = String::new();
    events
        .feed(
            b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n",
            |s| answer.push_str(&s),
        )
        .unwrap();
    assert!(events.finish().is_err());
    assert_eq!(answer, "partial");
    assert!(
        events
            .feed(
                b"data: {\"error\":{\"message\":\"sensitive data\"}}\n",
                |_| {}
            )
            .unwrap_err()
            .contains("provider")
    );
}
#[test]
fn malformed_empty_oversized_streams_fail_closed() {
    assert!(Events::default().feed(b"data: [DONE]\n", |_| {}).is_err());
    assert!(Events::default().feed(b"data: {broken}\n", |_| {}).is_err());
    assert!(
        Events::default()
            .feed(&vec![b'a'; 256 * 1024 + 1], |_| {})
            .is_err()
    );
}
#[test]
fn invalid_configuration_and_secret_rejected_before_external_io() {
    for endpoint in [
        "http://example.org",
        "file:///tmp/x",
        "https://user:secret@example.org",
        "https://example.org/#fragment",
    ] {
        let config = Configuration {
            endpoint: endpoint.into(),
            ..Configuration::default()
        };
        assert!(config.validate().is_err());
    }
    assert!(secret("invalid", None, &AtomicBool::new(false)).is_err());
    assert!(secret("groq", Some("secret\nheader"), &AtomicBool::new(false)).is_err());
}
#[test]
fn cancellation_reaps_worker_before_first_byte() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel = cancelled.clone();
    let started = std::time::Instant::now();
    let worker = std::thread::spawn(move || {
        process::run(
            Command::new("/usr/bin/sleep").arg("30"),
            Vec::new(),
            &cancel,
            Duration::from_secs(60),
            100,
            |_| Ok(()),
        )
    });
    std::thread::sleep(Duration::from_millis(40));
    cancelled.store(true, Ordering::Release);
    assert!(worker.join().unwrap().is_err());
    assert!(started.elapsed() < Duration::from_secs(2));
}
#[test]
fn local_protocol_fixture_observes_only_single_question_image_and_streams_answer() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let body = request(
        "owned synthetic question",
        &[0xff, 0xd8, 0xff, 0xd9],
        &Configuration::default(),
    );
    let sent = body.to_string();
    let expected = body.clone();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut headers = Vec::new();
        let mut byte = [0];
        while !headers.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            headers.push(byte[0]);
            assert!(headers.len() < 8192);
        }
        let headers = String::from_utf8(headers).unwrap();
        let length: usize = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|s| s.trim().parse().unwrap())
            })
            .unwrap();
        let mut received = vec![0; length];
        stream.read_exact(&mut received).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&received).unwrap(),
            expected
        );
        let answer = b"data: {\"choices\":[{\"delta\":{\"content\":\"**Synthetic** answer\"}}]}\n\ndata: [DONE]\n\n";
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", answer.len()).unwrap();
        for byte in answer {
            stream.write_all(&[*byte]).unwrap();
        }
    });
    // Loopback transport is constructed only in this test; product accepts HTTPS only.
    let mut response = ureq::post(format!("http://{address}/"))
        .header("Content-Type", "application/json")
        .send(sent)
        .unwrap();
    let mut events = Events::default();
    let mut answer = String::new();
    let mut byte = [0];
    let mut reader = response.body_mut().as_reader();
    while reader.read(&mut byte).unwrap() != 0 {
        events.feed(&byte, |s| answer.push_str(&s)).unwrap();
    }
    events.finish().unwrap();
    assert_eq!(answer, "**Synthetic** answer");
    server.join().unwrap();
}

#[test]
fn oversized_completed_events_are_rejected_before_emitting_text() {
    let event = format!(
        "data: {}\n",
        serde_json::json!({"choices":[{"delta":{"content":"x".repeat(256 * 1024)}}]})
    );
    for split in [0, event.len() - 1] {
        let mut events = Events::default();
        let mut deltas = Vec::new();
        let first = events.feed(&event.as_bytes()[..split], |s| deltas.push(s));
        let last = events.feed(&event.as_bytes()[split..], |s| deltas.push(s));
        assert!(first.is_err() || last.is_err());
        assert!(deltas.is_empty());
    }
}
#[test]
fn cancellation_before_answer_start_never_opens_secret_service_or_cloud() {
    let answer = Answer::start(
        "fixture".into(),
        vec![],
        Configuration {
            enabled: true,
            ..Default::default()
        },
        Arc::new(AtomicBool::new(true)),
    );
    match answer.updates.recv_timeout(Duration::from_secs(1)).unwrap() {
        Update::Finished(Err(error)) => assert_eq!(error, "Ask cancelled"),
        _ => panic!("cancelled request must finish without an answer"),
    }
}
