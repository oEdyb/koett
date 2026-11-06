import Foundation

struct UpdateConfiguration {
    let feed: URL
    let permitsLoopback: Bool

    init?(info: [String: Any]) {
        guard info["SURequireSignedFeed"] as? Bool == true,
              info["SUVerifyUpdateBeforeExtraction"] as? Bool == true,
              (info["SUSignedFeedFailureExpirationInterval"] as? NSNumber)?.doubleValue == 0 else { return nil }
        guard let raw = info["SUFeedURL"] as? String, let feed = URL(string: raw),
              let key = info["SUPublicEDKey"] as? String, Data(base64Encoded: key)?.count == 32 else { return nil }
        let local = info["KoettLocalUpdateTesting"] as? Bool == true
        guard Self.accepts(feed, permitsLoopback: local) else { return nil }
        self.feed = feed
        permitsLoopback = local
    }

    func acceptsArchive(_ url: URL) -> Bool { Self.accepts(url, permitsLoopback: permitsLoopback) }

    private static func accepts(_ url: URL, permitsLoopback: Bool) -> Bool {
        guard url.user == nil, url.password == nil, url.fragment == nil, let host = url.host, !host.isEmpty else { return false }
        return url.scheme == "https" || (permitsLoopback && url.scheme == "http" && ["127.0.0.1", "::1"].contains(host))
    }
}

@MainActor
final class UpdateInstallGate {
    private let isIdle: () -> Bool
    private(set) var isInstalling = false
    private var pending: (() -> Void)?
    private var generation: UInt64 = 0
    private var noticeShown = false
    func takeReservationNotice() -> Bool {
        guard isInstalling, !noticeShown else { return false }
        noticeShown = true
        return true
    }
    var hasReservationNotice: Bool { noticeShown && isInstalling }
    var isPending: Bool { pending != nil }

    init(isIdle: @escaping () -> Bool) { self.isIdle = isIdle }

    func deferInstallation(_ action: @escaping () -> Void) {
        generation &+= 1
        pending = action
        resumeIfIdle()
    }

    func reserveIfIdle() -> Bool {
        guard isIdle() else { return false }
        isInstalling = true
        return true
    }

    func resumeIfIdle() {
        guard let action = pending, reserveIfIdle() else { return }
        pending = nil
        let expected = generation
        DispatchQueue.main.async { [weak self] in
            guard let self, self.generation == expected else { return }
            guard self.isIdle() else {
                self.isInstalling = false
                self.pending = action
                return
            }
            action()
        }
    }

    func reset() { generation &+= 1; pending = nil; isInstalling = false; noticeShown = false }
}
