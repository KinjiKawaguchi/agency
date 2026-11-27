import AgencyAPI
import AgencyKit
import Foundation
import Testing
@testable import AgencyModel

@Suite("Node and app compatibility")
struct NodeCompatibilityTests {
    private func judge(
        _ about: About?, clientApi: Int = 2, version: String? = "1.2.0", platform: ClientIdentity.Platform = .ios
    ) -> NodeCompatibility {
        NodeCompatibility.judge(about: about, clientApi: clientApi, appVersion: version, platform: platform)
    }

    private func about(current: Int, oldest: Int, floor: [String: String]? = nil) -> About {
        About(capabilities: [], clientApi: ClientApiRange(current: current, oldest: oldest), oldestApp: floor)
    }

    @Test("an app inside the node's range, with no floor, is compatible")
    func compatible() {
        #expect(judge(about(current: 2, oldest: 2)) == .compatible)
        #expect(judge(about(current: 3, oldest: 1)) == .compatible)
        #expect(judge(about(current: 2, oldest: 1)) == .compatible)
        #expect(judge(about(current: 2, oldest: 2), clientApi: 2) == .compatible)
    }

    @Test("a node that serves only older versions of the Client API than the app speaks is too old")
    func nodeTooOld() {
        #expect(judge(about(current: 1, oldest: 1), clientApi: 2) == .nodeTooOld)
        #expect(judge(about(current: 2, oldest: 1), clientApi: 3) == .nodeTooOld)
    }

    @Test("a node that no longer serves the version the app speaks leaves the app too old")
    func appTooOldByRange() {
        #expect(judge(about(current: 3, oldest: 3), clientApi: 2) == .appTooOld)
        #expect(judge(about(current: 4, oldest: 3), clientApi: 1) == .appTooOld)
    }

    @Test("an app older than the node's floor for its platform is too old, and one at the floor is not")
    func appTooOldByFloor() {
        let floors = about(current: 2, oldest: 1, floor: ["ios": "1.3.0"])
        #expect(judge(floors, version: "1.2.0") == .appTooOld)
        #expect(judge(floors, version: "1.2.9") == .appTooOld)
        #expect(judge(floors, version: "1.3.0") == .compatible)
        #expect(judge(floors, version: "1.10.0") == .compatible, "numbers, not text: 10 is more than 3")
        #expect(judge(floors, version: "2.0.0") == .compatible)
        #expect(judge(floors, version: "0.9.9") == .appTooOld)
    }

    @Test("the floor that counts is the one for this app's platform")
    func floorOfThePlatform() {
        let floors = about(current: 2, oldest: 1, floor: ["macos": "9.0.0"])
        #expect(judge(floors, platform: .ios) == .compatible)
        #expect(judge(floors, platform: .macos) == .appTooOld)
    }

    @Test("with no floor, no range, no answer or no version of its own the app says nothing")
    func nothingToCompare() {
        #expect(judge(About(capabilities: [])) == .compatible, "a node that says neither the range nor a floor, as a release before it did")
        #expect(judge(nil) == .compatible)
        #expect(judge(About(capabilities: [], oldestApp: [:])) == .compatible)
        #expect(judge(about(current: 2, oldest: 1, floor: ["ios": "9.0.0"]), version: nil) == .compatible)
    }

    @Test("a floor that is not numbers is no floor, and a missing number is a zero")
    func versionForms() {
        for odd in ["", "x", "1.x", "1..0", "1.0-beta", "-1.0", "99999999999.0", "１.0"] {
            #expect(judge(about(current: 2, oldest: 1, floor: ["ios": odd]), version: "0.0.1") == .compatible, Comment(rawValue: odd))
        }
        #expect(judge(about(current: 2, oldest: 1, floor: ["ios": "1.2"]), version: "1.2.0") == .compatible)
        #expect(judge(about(current: 2, oldest: 1, floor: ["ios": "1.2.0"]), version: "1.1.9") == .appTooOld)
        #expect(judge(about(current: 2, oldest: 1, floor: ["ios": "2"]), version: "1.9.9") == .appTooOld)
        #expect(judge(about(current: 2, oldest: 1, floor: ["ios": "1.2.0.1"]), version: "1.2.0") == .appTooOld)
    }

    @Test("what the owner is told names the part to update, and nothing is said when it fits")
    func notices() {
        #expect(NodeCompatibility.compatible.notice == nil)
        #expect(NodeCompatibility.nodeTooOld.notice?.contains("ノードを更新") == true)
        #expect(NodeCompatibility.appTooOld.notice?.contains("アプリを更新") == true)
    }
}
