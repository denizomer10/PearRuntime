// Clean-room: from the public UIAccessibility documentation.
import AppKit
import Foundation

@_cdecl("UIAccessibilityIsVoiceOverRunning")
public func UIAccessibilityIsVoiceOverRunning() -> Bool {
    NSWorkspace.shared.isVoiceOverEnabled
}

@_cdecl("UIAccessibilityPostNotification")
public func UIAccessibilityPostNotification(_ notification: UInt32, _ argument: AnyObject?) {
    // No-op on macOS host for now.
}

@_cdecl("UIAccessibilityRegisterGestureConflictWithZoom")
public func UIAccessibilityRegisterGestureConflictWithZoom() {
    // No-op on macOS host.
}
