#if canImport(JavaScriptCore)
import Foundation
import JavaScriptCore

/// Tests run in parallel, and JavaScriptCore traps when two virtual machines start at the same
/// moment (in its allocator and lock setup), so every context is made under one lock.
enum JavaScript {
    private static let lock = NSLock()

    static func makeContext() -> JSContext {
        lock.lock()
        defer { lock.unlock() }
        return JSContext()!
    }
}
#endif
