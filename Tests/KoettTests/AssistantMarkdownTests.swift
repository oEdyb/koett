@testable import Koett
import AppKit
import WebKit
import XCTest

final class AssistantMarkdownTests: XCTestCase {
    func testRendererIncludesAllLocalResources() throws {
        XCTAssertTrue(AssistantRendererResources.isComplete)

        let directory = try XCTUnwrap(
            AssistantRendererResources.directoryURL
        )
        for fileName in AssistantRendererResources.requiredFileNames {
            XCTAssertTrue(FileManager.default.fileExists(
                atPath: directory.appendingPathComponent(fileName).path
            ))
        }
    }

    func testRendererUsesSanitizedMarkdownMathAndCodeHighlighting() throws {
        let directory = try XCTUnwrap(
            AssistantRendererResources.directoryURL
        )
        let script = try String(
            contentsOf: directory.appendingPathComponent("renderer.js"),
            encoding: .utf8
        )

        XCTAssertTrue(script.contains("marked.parse"))
        XCTAssertTrue(script.contains("DOMPurify.sanitize"))
        XCTAssertTrue(script.contains("renderMathInElement"))
        XCTAssertTrue(script.contains("hljs.highlightElement"))
        XCTAssertTrue(script.contains("trust: false"))
    }

    func testRendererBlocksRuntimeNetworkAccess() throws {
        let pageURL = try XCTUnwrap(AssistantRendererResources.pageURL)
        let page = try String(contentsOf: pageURL, encoding: .utf8)

        XCTAssertTrue(page.contains("default-src 'none'"))
        XCTAssertTrue(page.contains("connect-src 'none'"))
        XCTAssertFalse(page.contains("https://"))
        XCTAssertFalse(page.contains("http://"))
    }

    @MainActor
    func testWebKitRendersTableMathAndHighlightedCodeSafely() async throws {
        let webView = try await loadedRenderer()

        let result = try await webView.callAsyncJavaScript(
            """
            window.renderMarkdown(markdown);
            return JSON.stringify({
              table: document.querySelector("table") !== null,
              math: document.querySelector(".katex") !== null,
              code: document.querySelector("code.hljs") !== null,
              script: document.querySelector("#content script") !== null,
              eventHandler: document.querySelector("#content [onerror]") !== null
            });
            """,
            arguments: [
                "markdown": """
                | A | B |
                |---|---|
                | 1 | 2 |

                $$E = mc^2$$

                ```swift
                let ready = true
                ```

                <script>window.bad = true</script>
                <img src=x onerror="window.bad = true">
                """,
            ],
            in: nil,
            contentWorld: .page
        )
        let json = try XCTUnwrap(result as? String)
        let state = try JSONDecoder().decode(
            RenderedState.self,
            from: Data(json.utf8)
        )

        XCTAssertTrue(state.table)
        XCTAssertTrue(state.math)
        XCTAssertTrue(state.code)
        XCTAssertFalse(state.script)
        XCTAssertFalse(state.eventHandler)
    }

    @MainActor
    func testFastStreamRendersBeforeStreamFinishes() async throws {
        let webView = try await loadedRenderer()
        let view = AssistantMarkdownView(source: "")
        let coordinator = view.makeCoordinator()
        coordinator.webView(webView, didFinish: nil)

        let producer = Task { @MainActor in
            for index in 1...60 {
                coordinator.update(
                    source: "Streaming chunk \(index)",
                    in: webView
                )
                try? await Task.sleep(for: .milliseconds(5))
            }
        }

        try await Task.sleep(for: .milliseconds(120))
        let visibleText = try await webView.evaluateJavaScript(
            "document.getElementById('content').textContent"
        ) as? String

        XCTAssertFalse(visibleText?.isEmpty ?? true)
        XCTAssertNotEqual(visibleText, "Streaming chunk 60")
        await producer.value
        coordinator.stop()
    }

    @MainActor
    func testRendererBackgroundIsTransparent() async throws {
        let webView = try await loadedRenderer()
        webView.setFrameSize(NSSize(width: 436, height: 500))
        _ = try await webView.callAsyncJavaScript(
            "window.renderMarkdown(markdown)",
            arguments: [
                "markdown": """
                Here’s the short answer: **Cartesia should feel instant.**

                ## Why

                - The connection opens while you speak.
                - The first complete sentence starts playing immediately.

                | Part | Result |
                |---|---|
                | First audio | Under 100 ms |

                Inline math works: $E = mc^2$.
                """,
            ],
            in: nil,
            contentWorld: .page
        )
        try await Task.sleep(for: .milliseconds(150))

        let image = try await webView.takeSnapshot(configuration: nil)
        let tiff = try XCTUnwrap(image.tiffRepresentation)
        let bitmap = try XCTUnwrap(NSBitmapImageRep(data: tiff))
        let background = try XCTUnwrap(bitmap.colorAt(
            x: bitmap.pixelsWide - 12,
            y: bitmap.pixelsHigh - 12
        ))
        XCTAssertLessThan(background.alphaComponent, 0.05)
    }

    @MainActor
    private func loadedRenderer() async throws -> WKWebView {
        let pageURL = try XCTUnwrap(AssistantRendererResources.pageURL)
        let directory = try XCTUnwrap(
            AssistantRendererResources.directoryURL
        )
        let webView = WKWebView(
            frame: .zero,
            configuration: WKWebViewConfiguration()
        )
        AssistantMarkdownView.makeTransparent(webView)
        let navigation = NavigationWaiter()
        webView.navigationDelegate = navigation
        webView.loadFileURL(pageURL, allowingReadAccessTo: directory)
        await fulfillment(of: [navigation.finished], timeout: 3)
        return webView
    }
}

private struct RenderedState: Decodable {
    let table: Bool
    let math: Bool
    let code: Bool
    let script: Bool
    let eventHandler: Bool
}

@MainActor
private final class NavigationWaiter: NSObject, WKNavigationDelegate {
    let finished = XCTestExpectation(description: "Renderer loaded")

    func webView(
        _ webView: WKWebView,
        didFinish navigation: WKNavigation!
    ) {
        finished.fulfill()
    }
}
