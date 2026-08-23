import AppKit
import SwiftUI
import WebKit

enum AssistantRendererResources {
    static let requiredFileNames = [
        "index.html",
        "renderer.css",
        "renderer.js",
        "marked.js",
        "dompurify.js",
        "katex.js",
        "katex-auto-render.js",
        "katex.css",
        "highlight.js",
        "highlight.css",
    ]

    static var directoryURL: URL? {
        if let installedURL = Bundle.main.resourceURL?
            .appendingPathComponent("Koett_Koett.bundle")
            .appendingPathComponent("AssistantRenderer"),
           FileManager.default.fileExists(
               atPath: installedURL.appendingPathComponent("index.html").path
           ) {
            return installedURL
        }

        return Bundle.module.url(
            forResource: "AssistantRenderer",
            withExtension: nil
        )
    }

    static var pageURL: URL? {
        directoryURL?.appendingPathComponent("index.html")
    }

    static var isComplete: Bool {
        guard let directoryURL else { return false }
        return requiredFileNames.allSatisfy { fileName in
            FileManager.default.fileExists(
                atPath: directoryURL.appendingPathComponent(fileName).path
            )
        }
    }
}

struct AssistantMarkdownView: NSViewRepresentable {
    let source: String

    static func makeTransparent(_ webView: WKWebView) {
        webView.setValue(false, forKey: "drawsBackground")
        webView.underPageBackgroundColor = .clear
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false

        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = context.coordinator
        webView.allowsLinkPreview = false
        Self.makeTransparent(webView)

        if let pageURL = AssistantRendererResources.pageURL,
           let directoryURL = AssistantRendererResources.directoryURL {
            webView.loadFileURL(pageURL, allowingReadAccessTo: directoryURL)
        } else {
            webView.loadHTMLString(
                "<body style='color:white;background:transparent'>Renderer unavailable.</body>",
                baseURL: nil
            )
        }
        context.coordinator.update(source: source, in: webView)
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        context.coordinator.parent = self
        context.coordinator.update(source: source, in: webView)
    }

    static func dismantleNSView(
        _ webView: WKWebView,
        coordinator: Coordinator
    ) {
        coordinator.stop()
        webView.navigationDelegate = nil
        webView.stopLoading()
    }

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate {
        var parent: AssistantMarkdownView

        private var pendingSource = ""
        private var lastRenderedSource: String?
        private var isLoaded = false
        private var renderTask: Task<Void, Never>?

        init(parent: AssistantMarkdownView) {
            self.parent = parent
        }

        func update(source: String, in webView: WKWebView) {
            pendingSource = source
            guard isLoaded else { return }
            scheduleRender(in: webView, delayMilliseconds: 35)
        }

        func stop() {
            renderTask?.cancel()
            renderTask = nil
            isLoaded = false
        }

        func webView(
            _ webView: WKWebView,
            didFinish navigation: WKNavigation!
        ) {
            isLoaded = true
            scheduleRender(in: webView, delayMilliseconds: 0)
        }

        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping @MainActor @Sendable (
                WKNavigationActionPolicy
            ) -> Void
        ) {
            guard navigationAction.navigationType == .linkActivated else {
                decisionHandler(.allow)
                return
            }

            if let url = navigationAction.request.url,
               let scheme = url.scheme?.lowercased(),
               ["https", "http", "mailto"].contains(scheme) {
                NSWorkspace.shared.open(url)
            }
            decisionHandler(.cancel)
        }

        private func scheduleRender(
            in webView: WKWebView,
            delayMilliseconds: Int
        ) {
            guard renderTask == nil else { return }
            renderTask = Task { @MainActor [weak self, weak webView] in
                if delayMilliseconds > 0 {
                    try? await Task.sleep(
                        for: .milliseconds(delayMilliseconds)
                    )
                }
                guard !Task.isCancelled,
                      let self,
                      let webView else { return }
                renderTask = nil
                render(source: pendingSource, in: webView)
            }
        }

        private func render(source: String, in webView: WKWebView) {
            guard source != lastRenderedSource else { return }
            lastRenderedSource = source
            webView.callAsyncJavaScript(
                "window.renderMarkdown(markdown)",
                arguments: ["markdown": source],
                in: nil,
                in: .page,
                completionHandler: nil
            )
        }
    }
}
