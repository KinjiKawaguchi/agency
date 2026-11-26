import Foundation
import Testing
@testable import AgencyProtocol

@Suite("Sha256")
struct Sha256Tests {
    @Test("the digest of abc is the standard one, and of nothing is the standard one")
    func vectors() {
        func hex(_ data: Data) -> String { data.map { String(format: "%02x", $0) }.joined() }
        #expect(hex(Sha256.digest(Data("abc".utf8))) == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        #expect(hex(Sha256.digest(Data())) == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
    }
}
