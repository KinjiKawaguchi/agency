@testable import AgencyAPI
@testable import AgencyModel
import Testing

@Suite("PushDeviceChoice")
struct PushDeviceChoiceTests {
    private let capable = About(capabilities: ["webPush", "pushDevice"])
    private let older = About(capabilities: ["webPush"])

    @Test("a node that lists pushDevice is sent this phone's id once it is in the list")
    func named() {
        #expect(PushDeviceChoice.decide(about: capable, thisDeviceID: "d-me", registered: nil) == .withDevice("d-me"))
    }

    @Test("a node that does not list it is sent none: any other answers 400 for the field")
    func notCapable() {
        #expect(PushDeviceChoice.decide(about: older, thisDeviceID: "d-me", registered: nil) == .withoutDevice)
        #expect(PushDeviceChoice.decide(about: older, thisDeviceID: nil, registered: "d-me") == .withoutDevice)
        #expect(PushDeviceChoice.decide(about: About(capabilities: []), thisDeviceID: nil, registered: nil) == .withoutDevice)
    }

    @Test("what the node says of itself is waited for, not guessed")
    func unread() {
        #expect(PushDeviceChoice.decide(about: nil, thisDeviceID: "d-me", registered: nil) == .notYet)
    }

    @Test("a phone that is not in the list is not registered on a node that ties registrations to devices: an unnamed one would outlive its removal")
    func notAddedYet() {
        #expect(PushDeviceChoice.decide(about: capable, thisDeviceID: nil, registered: nil) == .notYet)
    }

    @Test("a phone that was named and has left the list keeps its registration's device, and is not registered again without it")
    func removed() {
        #expect(PushDeviceChoice.decide(about: capable, thisDeviceID: nil, registered: "d-me") == .withDevice("d-me"))
    }
}
