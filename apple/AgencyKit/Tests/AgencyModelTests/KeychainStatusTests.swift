@testable import AgencyModel
import Testing

@Suite("KeychainStatus")
struct KeychainStatusTests {
    @Test("only errSecItemNotFound is \"nothing is stored\": every other failure to read says nothing of whether a key is there")
    func read() {
        #expect(KeychainStatus.read(status: 0, hasData: true) == .found)
        #expect(KeychainStatus.read(status: -25300, hasData: false) == .notFound)
        for status: Int32 in [-25308 /* locked */, -25291, -25293, -34018, -67585, -50, 1] {
            #expect(KeychainStatus.read(status: status, hasData: false) == .failed(status: status), "\(status)")
        }
        #expect(KeychainStatus.read(status: 0, hasData: false) == .failed(status: KeychainStatus.decode), "success with no data is not an empty Keychain")
        #expect(KeychainStatus.read(status: -25300, hasData: true) == .notFound)
    }

    @Test("a handle is added where there is none; one that is there is a duplicate and is left alone; any other failure fails")
    func add() {
        #expect(KeychainStatus.add(status: 0) == .added)
        #expect(KeychainStatus.add(status: -25299) == .duplicate)
        for status: Int32 in [-25300, -25308, -34018, -108, 7] { #expect(KeychainStatus.add(status: status) == .failed(status: status), "\(status)") }
    }

    @Test("deleting what is not there is done, and a delete the Keychain refuses is a failure the person is told of")
    func delete() {
        #expect(KeychainStatus.delete(status: 0) == .deleted)
        #expect(KeychainStatus.delete(status: -25300) == .nothingToDelete)
        for status: Int32 in [-25308, -25293, -34018, -25299, 7] { #expect(KeychainStatus.delete(status: status) == .failed(status: status), "\(status)") }
    }
}
