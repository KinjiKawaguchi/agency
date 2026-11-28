@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@MainActor
@Suite("DeskStore")
struct DeskStoreTests {
    let source = FakeSource()
    let sender = FakeSigner()
    let submitter = FakeSubmitter()
    let storage = MemoryStorage()
    let kept = MemoryOutboxStorage()
    let first = "acme:app:r-1"

    func makeStore(storage: MemoryStorage? = nil, standing: Standing = .loading) -> DeskStore {
        DeskStore(
            source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage ?? self.storage,
            retry: .never, standing: standing
        )
    }
}
