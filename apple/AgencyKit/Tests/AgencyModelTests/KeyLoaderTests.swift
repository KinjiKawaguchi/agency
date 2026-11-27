@testable import AgencyModel
import AgencyProtocol
import Foundation
import Testing

private struct Failure: Error, CustomStringConvertible {
    let description: String
}

/// A key store whose answers are scripted, and which counts what is done to it.
private final class ScriptedVault: KeyVault, @unchecked Sendable {
    private let lock = NSLock()
    private var reads: [KeyHandleRead]
    private var opens: [Bool]
    private var _readCount = 0
    private var _createCount = 0
    var createFails = false
    /// When set, `create` throws it instead (a handle stored between the read and the add).
    var createThrows: (any Error)?
    private let signer = try! SoftwareP256Signer(rawPrivateKey: Data(repeating: 0x0b, count: 32))

    init(reads: [KeyHandleRead], opens: [Bool] = [true]) {
        self.reads = reads
        self.opens = opens
    }

    var readCount: Int { lock.withLock { _readCount } }
    var createCount: Int { lock.withLock { _createCount } }

    func readHandle(host: String) -> KeyHandleRead {
        lock.withLock {
            _readCount += 1
            return reads.count > 1 ? reads.removeFirst() : reads[0]
        }
    }

    func open(_ handle: Data) throws -> any DeviceSigner {
        let works = lock.withLock { opens.count > 1 ? opens.removeFirst() : opens[0] }
        guard works else { throw Failure(description: "the handle does not open") }
        return signer
    }

    func create(host: String) throws -> any DeviceSigner {
        lock.withLock { _createCount += 1 }
        if let createThrows { throw createThrows }
        if createFails { throw Failure(description: "no Secure Enclave") }
        return signer
    }
}

private final class Log: @unchecked Sendable {
    private let lock = NSLock()
    private var _lines: [String] = []
    private var _pauses = 0
    var lines: [String] { lock.withLock { _lines } }
    var pauses: Int { lock.withLock { _pauses } }
    func add(_ line: String) { lock.withLock { _lines.append(line) } }
    func paused() { lock.withLock { _pauses += 1 } }
}

@Suite("KeyLoader")
struct KeyLoaderTests {
    private let handle = Data([1, 2, 3])
    private let transient = KeyHandleRead.failed(reason: "the phone is locked", isTransient: true)

    private func load(_ vault: ScriptedVault, attempts: Int = 3, log: Log = Log()) async -> KeyLoad {
        await KeyLoader(vault: vault, attempts: attempts, pause: { log.paused() }, record: { log.add($0) }).load(host: "node.example")
    }

    private func isReady(_ load: KeyLoad) -> Bool {
        if case .ready = load { true } else { false }
    }

    @Test("a stored key that opens is the key, and nothing is made")
    func stored() async {
        let vault = ScriptedVault(reads: [.found(handle)])
        #expect(isReady(await load(vault)) && vault.createCount == 0 && vault.readCount == 1)
    }

    @Test("a key is made only when there is none stored")
    func firstLaunch() async {
        let vault = ScriptedVault(reads: [.notFound])
        #expect(isReady(await load(vault)) && vault.createCount == 1)
    }

    @Test("when a key cannot be made it is said, and the desk can only be read")
    func cannotMake() async {
        let vault = ScriptedVault(reads: [.notFound])
        vault.createFails = true
        let log = Log()
        guard case .cannotMake(let reason) = await load(vault, log: log) else { Issue.record("should be cannotMake"); return }
        #expect(reason == "no Secure Enclave" && log.lines == ["The device key could not be made: no Secure Enclave"])
    }

    @Test("a read that fails for a reason that may pass is tried again, and a key that then reads is the key: nothing is made")
    func transientThenReads() async {
        let vault = ScriptedVault(reads: [transient, transient, .found(handle)])
        let log = Log()
        #expect(isReady(await load(vault, log: log)) && vault.createCount == 0 && vault.readCount == 3 && log.pauses == 2)
    }

    @Test("a read that keeps failing is reported with its reason and written to the log, and no key is made or replaced")
    func transientForever() async {
        let vault = ScriptedVault(reads: [transient])
        let log = Log()
        guard case .unusable(let problem) = await load(vault, log: log) else { Issue.record("should be unusable"); return }
        #expect(problem == KeyProblem(reason: "the phone is locked", isTransient: true))
        #expect(vault.createCount == 0 && vault.readCount == 3, "tried the number of times asked, and no more")
        #expect(log.lines == ["The device key is stored and cannot be used: the phone is locked"])
    }

    @Test("a refusal that will not pass is reported at once, and no key is made")
    func refused() async {
        let vault = ScriptedVault(reads: [.failed(reason: "authentication failed", isTransient: false)])
        guard case .unusable(let problem) = await load(vault) else { Issue.record("should be unusable"); return }
        #expect(!problem.isTransient && vault.readCount == 1 && vault.createCount == 0)
    }

    @Test("a handle that is stored and does not open is not a missing key: it is tried again, then reported, and never replaced")
    func doesNotOpen() async {
        let always = ScriptedVault(reads: [.found(handle)], opens: [false])
        guard case .unusable(let problem) = await load(always) else { Issue.record("should be unusable"); return }
        #expect(problem.reason == "the handle does not open" && always.createCount == 0 && always.readCount == 3)
        let once = ScriptedVault(reads: [.found(handle)], opens: [false, true])
        #expect(isReady(await load(once)) && once.createCount == 0)
    }

    @Test("a handle stored between the read and the add is read, not replaced and not reported as a key that could not be made")
    func storedMeanwhile() async {
        let vault = ScriptedVault(reads: [.notFound, .found(handle)])
        vault.createThrows = KeyCreateError.alreadyStored
        let log = Log()
        #expect(isReady(await load(vault, log: log)) && vault.createCount == 1 && vault.readCount == 2 && log.lines.isEmpty)
    }

    @Test("a handle that is said to be stored and is then not found is reported, and no second key is made")
    func storedThenGone() async {
        let vault = ScriptedVault(reads: [.notFound])
        vault.createThrows = KeyCreateError.alreadyStored
        guard case .unusable = await load(vault) else { Issue.record("should be unusable"); return }
        #expect(vault.createCount == 1, "one try to make a key, and no more")
    }

    @Test("a single attempt is the least there is")
    func attempts() async {
        let vault = ScriptedVault(reads: [transient])
        _ = await load(vault, attempts: 0)
        #expect(vault.readCount == 1)
    }

    @Test("the Keychain's own statuses that pass are the locked phone and a store that is busy, and no other")
    func statuses() async {
        for status: Int32 in [-25308, -25291, -67585, -36, -108, -49] { #expect(KeychainFailure.isTransient(status: status), "\(status)") }
        for status: Int32 in [-25300 /* not found */, -25293 /* auth failed */, -25299 /* duplicate */, -34018 /* missing entitlement */, -50] {
            #expect(!KeychainFailure.isTransient(status: status), "\(status)")
        }
    }
}
