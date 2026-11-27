@testable import AgencyAPI
@testable import AgencyModel
import Testing

@Suite("PushDeviceChoice")
struct PushDeviceChoiceTests {
    private let capable = About(capabilities: ["webPush", "pushDevice"])
    private let older = About(capabilities: ["webPush"])

    @Test("a node that lists pushDevice is sent this phone's id once it is in the list")
    func named() {
        #expect(PushDeviceChoice.device(about: capable, thisDeviceID: "d-me", registered: nil) == "d-me")
    }

    @Test("a node that does not list it, or has not been read, is sent none: any other answers 400 for the field")
    func notNamed() {
        #expect(PushDeviceChoice.device(about: older, thisDeviceID: "d-me", registered: nil) == nil)
        #expect(PushDeviceChoice.device(about: nil, thisDeviceID: "d-me", registered: nil) == nil)
        #expect(PushDeviceChoice.device(about: older, thisDeviceID: "d-me", registered: "d-me") == nil)
    }

    @Test("a phone that is not in the list yet names none: the node refuses a device that is not the account's")
    func notAddedYet() {
        #expect(PushDeviceChoice.device(about: capable, thisDeviceID: nil, registered: nil) == nil)
    }

    @Test("a phone that was named and has left the list keeps its registration's device, and is not registered again without it")
    func removed() {
        #expect(PushDeviceChoice.device(about: capable, thisDeviceID: nil, registered: "d-me") == "d-me")
    }
}
