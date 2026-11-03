import AppKit

/// Draw and pin while dictating: one transparent, non-activating panel per
/// screen. Holding the draw key makes the panels take the pointer: moving
/// leaves red ink, clicking drops a pin. Otherwise clicks pass through. The key is read from modifier-flag events
/// only, so the overlay never takes keyboard focus and typing keeps working.
@MainActor
final class SketchOverlay {
    struct Session {
        let id: UInt64
        let key: DrawKey
        let frames: SketchFrames
        /// Captured-audio seconds (paused time excluded), for speech linking.
        let audioClock: () -> TimeInterval
        /// Pins also read the selected text of the app under the click.
        let includeSelection: Bool
        let onUpdate: (SketchUpdate) -> Void
    }

    struct Display: Equatable {
        let id: CGDirectDisplayID
        let frame: CGRect
        let scale: CGFloat
    }

    /// The system calls the overlay makes, replaceable so tests run without
    /// showing windows or asking for Screen Recording.
    struct Environment {
        var displays: @MainActor () -> [Display]
        var capture: @MainActor (CGDirectDisplayID, CGFloat) async throws -> CGImage
        var modifiers: @MainActor () -> NSEvent.ModifierFlags
        var pointer: @MainActor () -> CGPoint
        var showPanel: @MainActor (NSPanel) -> Void

        @MainActor static var live: Environment {
            Environment(
                displays: {
                    NSScreen.screens.compactMap { screen in
                        SketchCapture.displayID(of: screen).map {
                            Display(id: $0, frame: screen.frame, scale: screen.backingScaleFactor)
                        }
                    }
                },
                capture: { try await SketchCapture.still(displayID: $0, scale: $1) },
                modifiers: { NSEvent.modifierFlags },
                pointer: { NSEvent.mouseLocation },
                showPanel: { $0.orderFrontRegardless() }
            )
        }
    }

    /// One display's panel. Sketches refer to surfaces by index, so a display
    /// change retires surfaces instead of removing them.
    private struct Surface {
        let panel: NSPanel
        let view: SketchInkView
        let frame: CGRect
        let displayID: CGDirectDisplayID
        let scale: CGFloat
        /// Top of the main display when this surface was made, for global points.
        let mainTop: CGFloat
        var active = true
    }

    private let environment: Environment
    private var layout: [Display] = []
    private var displayObserver: NSObjectProtocol?
    private var session: Session?
    private var board: SketchBoard?
    private var surfaces: [Surface] = []
    private var monitors: [Any] = []
    private var ticker: Timer?
    private(set) var failedCaptures = 0

    var isActive: Bool { session != nil }

    init(environment: Environment = .live) {
        self.environment = environment
    }

    /// Failed captures since the last call (read once, after delivery).
    func takeFailedCaptures() -> Int {
        defer { failedCaptures = 0 }
        return failedCaptures
    }

    func start(_ session: Session, startedAt: TimeInterval) {
        discard()
        self.session = session
        board = SketchBoard(startedAt: startedAt)
        failedCaptures = 0
        session.frames.setOpen(session.id, true)
        layout = environment.displays()
        addSurfaces(for: layout)
        displayObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.displaysChanged() }
        }
        let handler: (NSEvent) -> Void = { [weak self] _ in
            MainActor.assumeIsolated { self?.followKey() }
        }
        if let global = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged, handler: handler) {
            monitors.append(global)
        }
        if let local = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged, handler: { event in
            handler(event)
            return event
        }) {
            monitors.append(local)
        }
        followKey()
    }

    /// Recording stopped: open sketches close and are captured; the panels go
    /// away at once so nothing covers the paste target.
    func finish() {
        guard var board else { return }
        publish(board.finish(now: Self.now))
        self.board = board
        removeMonitors()
        stopTicker()
        surfaces.forEach { $0.panel.orderOut(nil) }
        startCaptures()
        closeIfDone()
    }

    /// Cancelled or failed: drop everything; late captures are ignored.
    func discard() {
        removeMonitors()
        ticker?.invalidate()
        ticker = nil
        surfaces.forEach { $0.panel.orderOut(nil) }
        surfaces = []
        board = nil
        session = nil
    }

    /// A monitor came or went, or the arrangement or resolution changed. Open
    /// sketches close and are captured from the panels they were drawn on, then
    /// every display gets a new panel. An unchanged layout keeps the hold going:
    /// this notification also fires for changes that move no display.
    func displaysChanged() {
        guard var board, !board.finishing else { return }
        let current = environment.displays()
        guard current != layout else { return }
        layout = current
        publish(board.release(now: Self.now))
        self.board = board
        setAcceptsPointer(false)
        startCaptures()
        for index in surfaces.indices {
            surfaces[index].active = false
            surfaces[index].panel.orderOut(nil)
        }
        addSurfaces(for: current)
        followKey()
        startTicker()
    }

    // MARK: Input

    private func followKey() {
        guard var board, let session, !board.finishing else { return }
        let draws = session.key.draws(environment.modifiers())
        if draws && !board.held {
            board.press()
            self.board = board
            setAcceptsPointer(true)
            startTicker()
        } else if !draws && board.held {
            publish(board.release(now: Self.now))
            self.board = board
            setAcceptsPointer(false)
            redraw()
            startCaptures()
            startTicker()
        }
    }

    func tick() {
        followKey()
        guard var board, let session else { return stopTicker() }
        if board.held {
            let location = environment.pointer()
            if let index = surfaces.firstIndex(where: { $0.active && NSMouseInRect(location, $0.frame, false) }) {
                let frame = surfaces[index].frame
                let point = CGPoint(x: location.x - frame.minX, y: frame.maxY - location.y)
                let updates = board.point(
                    screen: index, point, now: Self.now, audio: Float(session.audioClock())
                )
                self.board = board
                if !updates.isEmpty {
                    publish(updates)
                    surfaces[index].view.needsDisplay = true
                }
            }
        } else if board.hasFadingInk(at: Self.now) {
            redraw()
        } else {
            redraw()
            stopTicker()
        }
    }

    /// A click on a panel; it only arrives while the key is held.
    private func mouse(down: Bool, screen index: Int, at point: CGPoint, clickCount: Int) {
        guard var board, let session, index < surfaces.count else { return }
        let updates = down
            ? board.mouseDown(screen: index, point, now: Self.now, audio: Float(session.audioClock()), clickCount: clickCount)
            : board.mouseUp(now: Self.now)
        self.board = board
        publish(updates)
        redraw()
        if !down { startCaptures() }
        startTicker()
    }

    private func setAcceptsPointer(_ accepts: Bool) {
        for surface in surfaces { surface.panel.ignoresMouseEvents = !accepts }
    }

    // MARK: Capture

    private func startCaptures() {
        guard var board, let session else { return }
        let ready = board.takeReadyForCapture()
        self.board = board
        for group in ready where group.screen < surfaces.count {
            let surface = surfaces[group.screen]
            let logical = surface.frame.size
            let includeSelection = session.includeSelection
            let capture = environment.capture
            let global = group.pin.map {
                CGPoint(x: surface.frame.minX + $0.x, y: surface.mainTop - surface.frame.maxY + $0.y)
            }
            Task { @MainActor [weak self] in
                async let context = global.asyncMap { await PinContextReader.read(at: $0, includeSelection: includeSelection) }
                let result: Result<(CGImage, PinContext?), Error>
                do {
                    let still = try await capture(surface.displayID, surface.scale)
                    guard let frame = SketchSheet.keyframe(
                        screen: still, logical: logical, strokes: group.strokes, pin: group.pin
                    ) else {
                        throw NSError(domain: "Koett.Sketch", code: 2, userInfo: [
                            NSLocalizedDescriptionKey: "The sketch image could not be drawn.",
                        ])
                    }
                    result = .success((frame, await context ?? nil))
                } catch {
                    result = .failure(error)
                }
                self?.finishCapture(group.id, session: session.id, result: result)
            }
        }
    }

    private func finishCapture(
        _ id: UInt64, session sessionID: UInt64, result: Result<(CGImage, PinContext?), Error>
    ) {
        guard var board, let session, session.id == sessionID else { return }
        switch result {
        case .success(let (image, context)):
            if let group = board.captured(id) {
                session.frames.push(session.id, SketchFrame(
                    order: group.id, start: group.audioStart, image: image, context: context
                ))
            }
        case .failure(let error):
            failedCaptures += 1
            fputs("Sketch capture failed: \(error.localizedDescription)\n", stderr)
            publish(board.failed(id))
        }
        self.board = board
        redraw()
        closeIfDone()
    }

    /// Delivery gave up waiting: pending captures are dropped and renumbered,
    /// and their late results are ignored.
    func abandonPendingCaptures(_ sessionID: UInt64) {
        guard var board, let session, session.id == sessionID else { return }
        failedCaptures += board.groups.count { $0.phase != .captured }
        publish(board.abandonPending())
        self.board = board
        closeIfDone()
    }

    private func closeIfDone() {
        guard let board, let session, board.finishing, !board.isCapturing else { return }
        session.frames.setOpen(session.id, false)
        discard()
    }

    // MARK: Drawing

    private func publish(_ updates: [SketchUpdate]) {
        guard let session else { return }
        updates.forEach(session.onUpdate)
    }

    private func redraw() {
        surfaces.forEach { $0.view.needsDisplay = true }
    }

    private func startTicker() {
        guard ticker == nil else { return }
        let timer = Timer(timeInterval: 1.0 / 120, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
        RunLoop.main.add(timer, forMode: .common)
        ticker = timer
    }

    private func stopTicker() {
        ticker?.invalidate()
        ticker = nil
    }

    private func removeMonitors() {
        monitors.forEach(NSEvent.removeMonitor)
        monitors = []
        if let displayObserver { NotificationCenter.default.removeObserver(displayObserver) }
        displayObserver = nil
    }

    private func addSurfaces(for displays: [Display]) {
        let mainTop = displays.first?.frame.maxY ?? 0
        for display in displays {
            surfaces.append(makeSurface(display: display, index: surfaces.count, mainTop: mainTop))
        }
    }

    private func makeSurface(display: Display, index: Int, mainTop: CGFloat) -> Surface {
        let panel = PillPanel(
            contentRect: display.frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .screenSaver
        panel.ignoresMouseEvents = true
        panel.isReleasedWhenClosed = false
        panel.hidesOnDeactivate = false
        panel.collectionBehavior = PillPanel.overlayBehavior
        panel.animationBehavior = .none
        panel.setAccessibilityElement(false)
        let view = SketchInkView(frame: CGRect(origin: .zero, size: display.frame.size))
        view.groups = { [weak self] in
            guard let board = self?.board else { return [] }
            return board.groups.filter { $0.screen == index }
        }
        view.onMouse = { [weak self] down, point, clickCount in
            self?.mouse(down: down, screen: index, at: point, clickCount: clickCount)
        }
        panel.contentView = view
        panel.setFrame(display.frame, display: false)
        environment.showPanel(panel)
        return Surface(
            panel: panel, view: view, frame: display.frame, displayID: display.id, scale: display.scale, mainTop: mainTop
        )
    }

    private static var now: TimeInterval { ProcessInfo.processInfo.systemUptime }
}

/// Paints one screen's ink and pin pulses; captured sketches fade 1.5 s after
/// release. Receives clicks only while the draw key is held.
final class SketchInkView: NSView {
    var groups: () -> [SketchBoard.Group] = { [] }
    var onMouse: (_ down: Bool, _ point: CGPoint, _ clickCount: Int) -> Void = { _, _, _ in }

    override var isFlipped: Bool { true }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func mouseDown(with event: NSEvent) {
        onMouse(true, convert(event.locationInWindow, from: nil), event.clickCount)
    }

    override func mouseUp(with event: NSEvent) {
        onMouse(false, convert(event.locationInWindow, from: nil), event.clickCount)
    }

    override func rightMouseDown(with event: NSEvent) {}
    override func otherMouseDown(with event: NSEvent) {}

    override func draw(_ dirtyRect: NSRect) {
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        let now = ProcessInfo.processInfo.systemUptime
        for group in groups() {
            let alpha = group.alpha(at: now)
            guard alpha > 0 else { continue }
            if let pin = group.pin {
                let grown = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? 1 : 1.6 - 0.6 * alpha
                SketchSheet.drawPin(at: pin, in: context, radius: SketchSheet.pinRadius * grown, alpha: alpha)
            } else {
                SketchSheet.strokeInk(group.strokes, in: context, alpha: alpha)
            }
        }
    }
}

private extension Optional {
    func asyncMap<T>(_ transform: (Wrapped) async -> T) async -> T? {
        guard let self else { return nil }
        return await transform(self)
    }
}
