import Crypto
import Foundation
import Testing
@testable import AgencyProtocol

@Suite("P-256 device keys")
struct DeviceKeyTests {
    private let vectors = Vectors.shared
    private var commentText: Data { Data("agency/v1/envelope\n\(vectors.commentEnvelope.canonical)".utf8) }

    // MARK: The node's vector

    @Test("the key made from the vector's private scalar is the vector's public key, fingerprint, pairing code and id")
    func keyFromSeed() throws {
        let signer = try SoftwareP256Signer(rawPrivateKey: vectors.p256.privateKey)
        #expect(signer.publicKey.string == vectors.p256.publicKey)
        let fingerprint = signer.publicKey.fingerprint
        #expect(fingerprint.hex == vectors.p256.fingerprint)
        #expect(fingerprint.pairingCode == vectors.p256.pairingCode)
        #expect(fingerprint.deviceID == vectors.p256.deviceId)
    }

    @Test("the node's signature over the comment envelope verifies here")
    func nodeSignatureVerifies() throws {
        let key = try DevicePublicKey(string: vectors.p256.publicKey)
        #expect(key.verifies(vectors.p256.commentSignature, over: commentText))
    }

    @Test("the same signature over a changed body fails, and so does a changed signature")
    func tamper() throws {
        let key = try DevicePublicKey(string: vectors.p256.publicKey)
        #expect(!key.verifies(vectors.p256.commentSignature, over: Data("agency/v1/envelope\n\(vectors.commentEnvelope.canonical) ".utf8)))
        #expect(!key.verifies(vectors.p256.commentSignature, over: Data("agency/v1/device\n\(vectors.commentEnvelope.canonical)".utf8)))
        var changed = vectors.p256.commentSignature
        changed[10] ^= 0x01
        #expect(!key.verifies(changed, over: commentText))
        #expect(!key.verifies(Data(vectors.p256.commentSignature.dropLast()), over: commentText))
    }

    @Test("our own signature over the same text verifies, with the key from the vector")
    func ownSignature() async throws {
        let signer = try SoftwareP256Signer(rawPrivateKey: vectors.p256.privateKey)
        let signature = try await signer.sign(commentText, reason: "test")
        #expect(signature.count == 64)
        #expect(try DevicePublicKey(string: vectors.p256.publicKey).verifies(signature, over: commentText))
        // ECDSA is randomized: two signatures differ and both verify.
        let again = try await signer.sign(commentText, reason: "test")
        #expect(signature != again)
        #expect(signer.publicKey.verifies(again, over: commentText))
    }

    @Test("the Ed25519 vectors still verify through the same entry point")
    func ed25519() throws {
        let key = try DevicePublicKey(string: vectors.devicePublicKey.base64EncodedString())
        #expect(key == .ed25519(vectors.devicePublicKey))
        #expect(key.fingerprint.pairingCode == vectors.devicePairingCode)
        let text = Data("agency/v1/envelope\n\(vectors.envelope.canonical)".utf8)
        #expect(key.verifies(vectors.envelope.signature, over: text))
        #expect(!key.verifies(vectors.envelope.signature, over: commentText))
    }

    // MARK: What is a key

    @Test("a key reads back as the string it came from")
    func roundTrip() throws {
        let p256 = try DevicePublicKey(string: vectors.p256.publicKey)
        #expect(p256.string == vectors.p256.publicKey)
        #expect(p256.bytes.count == 65)
        let ed = try DevicePublicKey(string: vectors.devicePublicKey.base64EncodedString())
        #expect(ed.bytes.count == 32)
    }

    @Test("what is not exactly a key is refused", arguments: [
        "", "p256:", "p256:AAAA", "P256:BCCcMXtjeTXdPaHFT2NJXfsx+X0pPfCFcQMgWVyarLg/3eTGn8F6DHTCDMaSZi8EmJK6N6S6R9LHDNipmYY5H5s=",
        "p256: BCCcMXtjeTXdPaHFT2NJXfsx+X0pPfCFcQMgWVyarLg/3eTGn8F6DHTCDMaSZi8EmJK6N6S6R9LHDNipmYY5H5s=",
        "p256:BCCcMXtjeTXdPaHFT2NJXfsx+X0pPfCFcQMgWVyarLg/3eTGn8F6DHTCDMaSZi8EmJK6N6S6R9LHDNipmYY5H5s",
        "not base64!", "AAAA",
    ])
    func malformed(string: String) {
        #expect(throws: DeviceKeyError.malformedKey) { try DevicePublicKey(string: string) }
    }

    @Test("a compressed P-256 point, a point that is not on the curve, and a wrong prefix byte are refused")
    func pointForms() throws {
        let uncompressed = try DevicePublicKey(string: vectors.p256.publicKey).bytes
        let compressed = try P256.Signing.PublicKey(x963Representation: uncompressed).compressedRepresentation
        #expect(throws: DeviceKeyError.malformedKey) { try DevicePublicKey(string: "p256:" + compressed.base64EncodedString()) }
        var offCurve = uncompressed
        offCurve[64] ^= 0x01
        #expect(throws: DeviceKeyError.malformedKey) { try DevicePublicKey(string: "p256:" + offCurve.base64EncodedString()) }
        var wrongPrefix = uncompressed
        wrongPrefix[0] = 0x02
        #expect(throws: DeviceKeyError.malformedKey) { try DevicePublicKey(string: "p256:" + wrongPrefix.base64EncodedString()) }
        #expect(throws: DeviceKeyError.malformedKey) { try DevicePublicKey(string: "p256:" + (uncompressed + [0]).base64EncodedString()) }
    }

    @Test("an Ed25519 key of 31 or 33 bytes, or one written without padding, is refused")
    func edLengths() {
        for length in [31, 33] {
            #expect(throws: DeviceKeyError.malformedKey) { try DevicePublicKey(string: Data(repeating: 7, count: length).base64EncodedString()) }
        }
        #expect(throws: DeviceKeyError.malformedKey) { try DevicePublicKey(string: String(Data(repeating: 7, count: 32).base64EncodedString().dropLast())) }
    }

    @Test("one key has one string: a base64 text with other padding bits is refused")
    func nonCanonicalBase64() {
        // The last data character carries 4 bits of the key and 2 padding bits: 's' and 't' differ only in those.
        let p256 = vectors.p256.publicKey
        #expect(p256.hasSuffix("5s="))
        #expect(throws: DeviceKeyError.malformedKey) { try DevicePublicKey(string: String(p256.dropLast(2)) + "t=") }
        let ed = vectors.devicePublicKey.base64EncodedString()
        let last = ed[ed.index(ed.endIndex, offsetBy: -2)]
        let variant = Character(UnicodeScalar(last.unicodeScalars.first!.value + 1)!)
        #expect(throws: DeviceKeyError.malformedKey) { try DevicePublicKey(string: String(ed.dropLast(2)) + String(variant) + "=") }
    }

    @Test("a key of one kind never verifies a signature of the other")
    func kindsDoNotMix() throws {
        let p256 = try DevicePublicKey(string: vectors.p256.publicKey)
        let text = Data("agency/v1/envelope\n\(vectors.envelope.canonical)".utf8)
        #expect(!p256.verifies(vectors.envelope.signature, over: text))
        let ed = try DevicePublicKey(string: vectors.devicePublicKey.base64EncodedString())
        #expect(!ed.verifies(vectors.p256.commentSignature, over: commentText))
    }
}
