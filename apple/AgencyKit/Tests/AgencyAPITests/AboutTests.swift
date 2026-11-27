import Foundation
import Testing
@testable import AgencyAPI

@Suite("What a node says of itself")
struct AboutTests {
    private func decode(_ json: String) throws -> About {
        try JSONDecoder().decode(About.self, from: Data(json.utf8))
    }

    @Test("the Client API range and the floor are read beside the capabilities, and what is not known is ignored")
    func range() throws {
        let about = try decode(#"{"node":"0.5.0","contracts":{"federation":[1],"clientApi":{"current":3,"oldest":2}},"capabilities":["pushDevice","newName"],"oldestApp":{"ios":"1.3.0","macos":"1.2.0","android":"0.1.0"},"later":{"x":1}}"#)
        #expect(about.clientApi == ClientApiRange(current: 3, oldest: 2))
        #expect(about.oldestApp == ["ios": "1.3.0", "macos": "1.2.0", "android": "0.1.0"])
        #expect(about.has(.pushDevice) && about.capabilities.contains("newName"))
    }

    @Test("an answer from a node before the range, or without a floor, has none")
    func absent() throws {
        let older = try decode(#"{"capabilities":["webPush"]}"#)
        #expect(older.clientApi == nil && older.oldestApp == nil)
        let noFloor = try decode(#"{"node":"0.3.0","contracts":{"clientApi":{"current":1,"oldest":1}},"capabilities":[]}"#)
        #expect(noFloor.clientApi == ClientApiRange(current: 1, oldest: 1) && noFloor.oldestApp == nil)
    }

    @Test("a range or a floor in a form this app does not read costs the comparison, never the capabilities")
    func unreadable() throws {
        for bad in [
            #"{"capabilities":["pushDevice"],"contracts":{"clientApi":{"current":"two","oldest":1}}}"#,
            #"{"capabilities":["pushDevice"],"contracts":{"clientApi":2}}"#,
            #"{"capabilities":["pushDevice"],"contracts":7}"#,
            #"{"capabilities":["pushDevice"],"contracts":{"clientApi":{"current":2}}}"#,
        ] {
            let about = try decode(bad)
            #expect(about.has(.pushDevice) && about.clientApi == nil, Comment(rawValue: bad))
        }
        for bad in [
            #"{"capabilities":["pushDevice"],"oldestApp":["ios"]}"#,
            #"{"capabilities":["pushDevice"],"oldestApp":{"ios":3}}"#,
            #"{"capabilities":["pushDevice"],"oldestApp":"1.0.0"}"#,
        ] {
            let about = try decode(bad)
            #expect(about.has(.pushDevice) && about.oldestApp == nil, Comment(rawValue: bad))
        }
    }

    @Test("capabilities stay required: an answer without them is not an About")
    func capabilitiesRequired() {
        #expect(throws: DecodingError.self) { try decode(#"{"node":"0.3.0"}"#) }
    }
}
