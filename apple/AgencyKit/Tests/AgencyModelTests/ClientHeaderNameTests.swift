import AgencyAPI
import AgencyKit
import Testing

@Suite("Client header name")
struct ClientHeaderNameTests {
    @Test("the client that sends the header and the identity that makes it name the same header")
    func same() {
        #expect(NodeClient.clientHeader == ClientIdentity.header)
    }
}
