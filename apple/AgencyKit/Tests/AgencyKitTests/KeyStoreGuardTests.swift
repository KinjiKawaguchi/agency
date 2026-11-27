import Foundation
import Testing

/// The device key must never be replaced by the app on its own (agency#477): a new key is made only when the Keychain says there is none,
/// and its handle is added, never written over one. `SecItemUpdate` is what writes over a stored item, so it may live only in `save`, which
/// the key path does not use; and the Keychain's answers for the key are decided by `KeychainStatus`, which the tests cover.
@Suite("The device key is never replaced")
struct KeyStoreGuardTests {
    private static let keychain = "Shared/Native/KeychainStore.swift"
    /// What belongs to the device key: the Secure Enclave key and its store.
    private static let keyPath = ["Shared/Native/SecureEnclaveSigner.swift", "Shared/Native/SecureEnclaveVault.swift"]

    private static func root(from file: String = #filePath) -> URL {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<4 { url.deleteLastPathComponent() }
        return url
    }

    private static func source(_ relative: String) throws -> String {
        try String(contentsOf: root().appendingPathComponent(relative), encoding: .utf8)
    }

    /// The lines of each function of a type, by its name: a function starts at a line `    func name` or `    private func name`.
    static func functions(in text: String) -> [String: [String]] {
        let start = try! NSRegularExpression(pattern: #"^ {4}(?:private |static |fileprivate )*func (\w+)"#)
        var found: [String: [String]] = [:]
        var current: String?
        for line in text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init) {
            if let match = start.firstMatch(in: line, range: NSRange(line.startIndex..., in: line)), let name = Range(match.range(at: 1), in: line) {
                current = String(line[name])
            }
            if let current { found[current, default: []].append(line) }
        }
        return found
    }

    /// The functions whose body writes over a stored item.
    static func writersOverAStoredItem(in text: String) -> [String] {
        functions(in: text).filter { $0.value.contains { $0.contains("SecItemUpdate") && !$0.trimmingCharacters(in: .whitespaces).hasPrefix("//") } }.keys.sorted()
    }

    @Test("only `save` writes over a stored Keychain item, and the key path does not call it")
    func onlySaveReplaces() throws {
        #expect(Self.writersOverAStoredItem(in: try Self.source(Self.keychain)) == ["save"])
        for file in Self.keyPath {
            let text = try Self.source(file)
            #expect(!text.contains("SecItemUpdate"), "\(file) writes over a stored item")
            #expect(!text.contains(".save("), "\(file) calls `save`, which replaces what is stored")
            #expect(!text.contains(".load("), "\(file) calls `load`, which answers nothing for every failure and not only for \"none stored\"")
            #expect(!text.contains("loadOrCreate"), "\(file) makes a key on any failure to load one")
        }
    }

    @Test("the key's store reads, adds and deletes through what `KeychainStatus` decides, and makes its key with `add`")
    func decidedByStatus() throws {
        let store = Self.functions(in: try Self.source(Self.keychain))
        for name in ["read", "add", "delete"] {
            #expect(store[name]?.contains { $0.contains("KeychainStatus.\(name)(") } == true, "`\(name)` does not use KeychainStatus.\(name)")
        }
        let signer = try Self.source("Shared/Native/SecureEnclaveSigner.swift")
        #expect(signer.contains("service.add(") && signer.contains("service.read("))
    }

    @Test("the scan sees a write over a stored item when there is one")
    func seesOne() {
        let text = """
            struct Store {
                func save(_ data: Data) throws {
                    SecItemUpdate(a, b)
                }

                func add(_ data: Data) throws {
                    // SecItemUpdate is not used here
                    SecItemAdd(a, nil)
                }

                func read() {
                    SecItemUpdate(a, b)
                }
            }
            """
        #expect(Self.writersOverAStoredItem(in: text) == ["read", "save"])
    }
}
