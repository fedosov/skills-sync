import SwiftUI
import AppKit

struct WindowStateGeometry {
    static let minWindowWidth: CGFloat = 980
    static let minWindowHeight: CGFloat = 620
    static let minSidebarWidth: CGFloat = 300
    static let maxSidebarWidth: CGFloat = 420

    static func clampSidebarWidth(_ width: Double?) -> Double? {
        guard let width else { return nil }
        let clamped = min(max(CGFloat(width), minSidebarWidth), maxSidebarWidth)
        return Double(clamped)
    }

    static func validFrameRect(from state: AppWindowState, screensVisibleFrames: [CGRect]) -> CGRect? {
        let width = CGFloat(state.width)
        let height = CGFloat(state.height)
        guard width >= minWindowWidth, height >= minWindowHeight else {
            return nil
        }
        guard !screensVisibleFrames.isEmpty else {
            return nil
        }

        let candidate = CGRect(x: state.x, y: state.y, width: width, height: height)
        guard let screen = screensVisibleFrames.first(where: { $0.intersects(candidate) }) else {
            return nil
        }

        let finalWidth = min(candidate.width, screen.width)
        let finalHeight = min(candidate.height, screen.height)
        let maxX = screen.maxX - finalWidth
        let maxY = screen.maxY - finalHeight
        let finalX = min(max(candidate.origin.x, screen.minX), maxX)
        let finalY = min(max(candidate.origin.y, screen.minY), maxY)
        return CGRect(x: finalX, y: finalY, width: finalWidth, height: finalHeight)
    }
}

struct WindowStateCoordinator: NSViewRepresentable {
    let viewModel: AppViewModel

    func makeCoordinator() -> Coordinator {
        Coordinator(viewModel: viewModel)
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        DispatchQueue.main.async {
            context.coordinator.attach(to: view)
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            context.coordinator.attach(to: nsView)
        }
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.detach()
    }

    @MainActor
    final class Coordinator {
        private let viewModel: AppViewModel
        private var windowObservers: [NSObjectProtocol] = []
        private var splitViewObserver: NSObjectProtocol?
        private weak var currentWindow: NSWindow?
        private weak var splitView: NSSplitView?
        private var hasRestored = false
        private var pendingSaveWorkItem: DispatchWorkItem?

        init(viewModel: AppViewModel) {
            self.viewModel = viewModel
        }

        func attach(to view: NSView) {
            guard let window = view.window else { return }

            if currentWindow !== window {
                detachWindowObservers()
                currentWindow = window
                installWindowObservers(for: window)
                hasRestored = false
            }

            if splitView == nil || splitView !== findSplitView(in: window.contentView) {
                detachSplitViewObserver()
                splitView = findSplitView(in: window.contentView)
                installSplitViewObserver()
            }

            restoreStateIfNeeded()
        }

        func detach() {
            pendingSaveWorkItem?.cancel()
            pendingSaveWorkItem = nil
            detachWindowObservers()
            detachSplitViewObserver()
            currentWindow = nil
            splitView = nil
        }

        private func restoreStateIfNeeded() {
            guard !hasRestored, let window = currentWindow else { return }
            hasRestored = true

            if let savedWindowState = viewModel.restoredWindowState(),
               let frame = WindowStateGeometry.validFrameRect(
                from: savedWindowState,
                screensVisibleFrames: NSScreen.screens.map(\.visibleFrame)
               ) {
                window.setFrame(frame, display: false)
                if savedWindowState.isMaximized, !window.isZoomed {
                    window.zoom(nil)
                }
            }

            if let sidebarWidth = viewModel.restoredSidebarWidth() {
                applySidebarWidth(CGFloat(sidebarWidth))
            }

            scheduleSave()
        }

        private func installWindowObservers(for window: NSWindow) {
            let center = NotificationCenter.default
            windowObservers.append(
                center.addObserver(forName: NSWindow.didMoveNotification, object: window, queue: .main) { [weak self] _ in
                    Task { @MainActor in
                        self?.scheduleSave()
                    }
                }
            )
            windowObservers.append(
                center.addObserver(forName: NSWindow.didResizeNotification, object: window, queue: .main) { [weak self] _ in
                    Task { @MainActor in
                        self?.scheduleSave()
                    }
                }
            )
            windowObservers.append(
                center.addObserver(forName: NSWindow.willCloseNotification, object: window, queue: .main) { [weak self] _ in
                    Task { @MainActor in
                        self?.saveNow()
                    }
                }
            )
        }

        private func installSplitViewObserver() {
            guard let splitView else { return }
            splitViewObserver = NotificationCenter.default.addObserver(
                forName: NSSplitView.didResizeSubviewsNotification,
                object: splitView,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor in
                    self?.scheduleSave()
                }
            }
        }

        private func detachWindowObservers() {
            for observer in windowObservers {
                NotificationCenter.default.removeObserver(observer)
            }
            windowObservers.removeAll()
        }

        private func detachSplitViewObserver() {
            if let splitViewObserver {
                NotificationCenter.default.removeObserver(splitViewObserver)
            }
            splitViewObserver = nil
        }

        private func scheduleSave() {
            pendingSaveWorkItem?.cancel()
            let work = DispatchWorkItem { [weak self] in
                Task { @MainActor in
                    self?.saveNow()
                }
            }
            pendingSaveWorkItem = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2, execute: work)
        }

        private func saveNow() {
            guard let window = currentWindow else { return }
            let sidebarWidth = WindowStateGeometry.clampSidebarWidth(currentSidebarWidth())
            viewModel.persistWindowSnapshot(
                frame: window.frame,
                isZoomed: window.isZoomed,
                sidebarWidth: sidebarWidth
            )
        }

        private func currentSidebarWidth() -> Double? {
            guard let splitView, splitView.subviews.count >= 2 else { return nil }
            return Double(splitView.subviews[0].frame.width)
        }

        private func applySidebarWidth(_ width: CGFloat) {
            guard let splitView, splitView.subviews.count >= 2 else { return }
            splitView.setPosition(width, ofDividerAt: 0)
        }

        private func findSplitView(in root: NSView?) -> NSSplitView? {
            guard let root else { return nil }
            if let split = root as? NSSplitView {
                return split
            }
            for subview in root.subviews {
                if let split = findSplitView(in: subview) {
                    return split
                }
            }
            return nil
        }
    }
}
